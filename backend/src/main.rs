use std::net::SocketAddr;
use std::path::Path;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, Response},
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use compira_mcp::api;
use compira_mcp::{hash_password, generate_api_key, AppState, Config, Database, TaskManager, create_mcp_service};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,compira_mcp=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    std::fs::create_dir_all(&config.data_dir).map_err(|e| {
        anyhow::anyhow!("failed to create data dir {:?}: {e}", config.data_dir)
    })?;
    std::fs::create_dir_all(&config.repos_dir).map_err(|e| {
        anyhow::anyhow!("failed to create repos dir {:?}: {e}", config.repos_dir)
    })?;

    let db = Database::open(&config.database_url)?;
    db.log("info", "CompiraMCP server starting", None)?;
    let _ = db.cleanup_expired_sessions();
    let _ = db.bootstrap_llm_settings(&config);
    let _ = db.bootstrap_sync_settings(&config);
    let _ = db.bootstrap_auth_settings(&config);

    let admin_password = config
        .admin_password
        .clone()
        .unwrap_or_else(|| "admin123".into());
    if db.count_users().unwrap_or(0) == 0 {
        let hash = hash_password(&admin_password)?;
        if db.bootstrap_admin_user(&config.admin_username, &hash)? {
            tracing::warn!(
                "Created default admin user: {} / {}",
                config.admin_username,
                admin_password
            );
            tracing::warn!("Change the password after first login.");
        }
    }

    if let Some(admin_key) = &config.admin_api_key {
        if db.bootstrap_admin_key(admin_key)? {
            tracing::info!("Bootstrapped admin API key from COMPIRA_ADMIN_API_KEY");
        }
    } else if db.list_api_keys()?.is_empty() {
        let auto_key = generate_api_key();
        db.bootstrap_admin_key(&auto_key)?;
        tracing::warn!("No API keys found. Auto-generated admin key: {auto_key}");
        tracing::warn!("Save this key — it will not be shown again.");
    }

    let tasks = TaskManager::new(db.clone(), config.clone());
    let state = AppState::new(db.clone(), tasks, config.clone());

    let mcp_service = create_mcp_service(db.clone());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = api::routes(state.clone());

    let mcp_router = Router::new()
        .nest_service("/mcp", mcp_service)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            mcp_auth_middleware,
        ));

    let static_dir = std::env::var("COMPIRA_STATIC_DIR")
        .unwrap_or_else(|_| "../frontend/dist".into());

    let mut app = Router::new()
        .route("/", get(index))
        .nest("/api", api_router)
        .merge(mcp_router);

    let index_file = Path::new(&static_dir).join("index.html");
    if index_file.exists() {
        let static_service = ServeDir::new(&static_dir)
            .not_found_service(ServeFile::new(index_file));
        app = app.fallback_service(static_service);
        tracing::info!("Serving static files from {static_dir}");
    } else {
        tracing::warn!(
            "Static dir not found ({static_dir}), Admin UI static files disabled. \
             Run `npm run build` in frontend/ or use dev.bat with Vite on :5173."
        );
    }

    let app = app
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("CompiraMCP listening on http://{addr}");
    tracing::info!("  REST API: http://{addr}/api");
    tracing::info!("  MCP endpoint: http://{addr}/mcp");
    tracing::info!("  Admin UI: http://{addr}/");

    // Periodically purge expired login sessions.
    {
        let db_cleanup = db.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                ticker.tick().await;
                if let Err(e) = db_cleanup.cleanup_expired_sessions() {
                    tracing::warn!("session cleanup failed: {e}");
                }
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Shutting down...");
        })
        .await?;

    Ok(())
}

async fn mcp_auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
        });

    if let Some(k) = key {
        if state.verify_key(k).await {
            return Ok(next.run(request).await);
        }
        if let Some(admin) = &state.config.admin_api_key {
            if k == admin.as_str() {
                return Ok(next.run(request).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn index() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>CompiraMCP</title></head>
<body><h1>CompiraMCP</h1><p>Admin UI will be available after frontend build. API: /api, MCP: /mcp</p></body></html>"#,
    )
}
