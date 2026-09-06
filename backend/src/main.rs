use std::net::SocketAddr;
use std::path::PathBuf;

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
    let _ = db.bootstrap_cleanup_settings();

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

    let mcp_service = create_mcp_service(db.clone(), state.mcp_activity.clone());

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

    // API + MCP first; frontend is served at `/` and as SPA fallback (refresh-safe).
    let mut app = Router::new()
        .nest("/api", api_router)
        .merge(mcp_router);

    if let Some(static_dir) = resolve_static_dir() {
        let index_file = static_dir.join("index.html");
        let static_service = ServeDir::new(&static_dir)
            .append_index_html_on_directories(true)
            .not_found_service(ServeFile::new(index_file.clone()));
        // Prefer real admin UI at `/` (do not register stub route that shadows dist).
        app = app
            .route_service("/", ServeFile::new(index_file))
            .fallback_service(static_service);
        tracing::info!("Serving admin UI from {}", static_dir.display());
    } else {
        app = app.route("/", get(index_stub));
        tracing::warn!(
            "Admin UI static files not found. Run `npm run build` in frontend/, \
             set COMPIRA_STATIC_DIR, or use Vite on :5173 (dev.bat)."
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

    // Scheduled storage maintenance (orphan repos, old logs/tasks, WAL).
    {
        let db_maint = db.clone();
        let config_maint = config.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(600));
            loop {
                ticker.tick().await;
                let schedule = match db_maint.get_cleanup_schedule() {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("read cleanup schedule: {e}");
                        continue;
                    }
                };
                if !schedule.enabled {
                    continue;
                }
                let due = match &schedule.last_run_at {
                    None => true,
                    Some(iso) => match iso.parse::<chrono::DateTime<chrono::Utc>>() {
                        Ok(t) => {
                            let elapsed = chrono::Utc::now().signed_duration_since(t);
                            elapsed
                                >= chrono::Duration::hours(schedule.interval_hours as i64)
                        }
                        Err(_) => true,
                    },
                };
                if !due {
                    continue;
                }
                let opts = compira_mcp::storage::CleanupOptions::from(&schedule);
                let db2 = db_maint.clone();
                let cfg2 = config_maint.clone();
                match tokio::task::spawn_blocking(move || {
                    compira_mcp::storage::run_cleanup(&db2, &cfg2, opts)
                })
                .await
                {
                    Ok(Ok(r)) => {
                        let summary = compira_mcp::storage::format_cleanup_summary(&r);
                        let _ = db_maint.record_cleanup_run(
                            &compira_mcp::storage::now_rfc3339(),
                            &summary,
                        );
                        tracing::info!("Scheduled cleanup finished: {summary}");
                        let _ = db_maint.log("info", &format!("Scheduled cleanup: {summary}"), None);
                    }
                    Ok(Err(e)) => tracing::warn!("scheduled cleanup failed: {e}"),
                    Err(e) => tracing::warn!("scheduled cleanup join: {e}"),
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
        if let Some((id, name)) = state.resolve_key(k).await {
            let caller = compira_mcp::mcp::activity::McpCaller::mcp(Some(id), Some(name));
            return Ok(compira_mcp::mcp::activity::CURRENT_MCP_CALLER
                .scope(caller, next.run(request))
                .await);
        }
        if let Some(admin) = &state.config.admin_api_key {
            if k == admin.as_str() {
                let caller = compira_mcp::mcp::activity::McpCaller::mcp(
                    None,
                    Some("env-admin".into()),
                );
                return Ok(compira_mcp::mcp::activity::CURRENT_MCP_CALLER
                    .scope(caller, next.run(request))
                    .await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn index_stub() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>CompiraMCP</title>
<style>body{font-family:system-ui;max-width:560px;margin:48px auto;padding:0 16px;line-height:1.5;color:#1a1a1a}
code{background:#f2f2f2;padding:2px 6px;border-radius:4px}</style></head>
<body>
<h1>CompiraMCP</h1>
<p>管理后台静态文件尚未加载。</p>
<ul>
<li>开发：运行 <code>dev.bat</code>，浏览器打开 <code>http://127.0.0.1:5173</code></li>
<li>生产：在 <code>frontend/</code> 执行 <code>npm run build</code> 后重启后端，直接访问本机端口根路径</li>
<li>或设置环境变量 <code>COMPIRA_STATIC_DIR</code> 指向含 <code>index.html</code> 的目录</li>
</ul>
<p>API：<code>/api</code> · MCP：<code>/mcp</code></p>
</body></html>"#,
    )
}

/// Locate built admin UI (`index.html`). Tries env, cwd-relative paths, and next to the binary.
fn resolve_static_dir() -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(raw) = std::env::var("COMPIRA_STATIC_DIR") {
        candidates.push(PathBuf::from(raw));
    }

    candidates.push(PathBuf::from("../frontend/dist"));
    candidates.push(PathBuf::from("frontend/dist"));
    candidates.push(PathBuf::from("static"));
    candidates.push(PathBuf::from("./static"));

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("static"));
            candidates.push(parent.join("frontend").join("dist"));
            // cargo run: target/debug/compira-mcp → ../../../frontend/dist
            candidates.push(parent.join("..").join("..").join("..").join("frontend").join("dist"));
        }
    }

    for cand in candidates {
        let index = cand.join("index.html");
        if index.is_file() {
            // Prefer absolute path for logging / ServeDir stability on Windows.
            return Some(cand.canonicalize().unwrap_or(cand));
        }
    }
    None
}
