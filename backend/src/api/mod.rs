mod auth;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::db::generate_api_key;
use crate::state::{AppState, AuthUser};
use crate::tasks::library_local_path;

pub fn routes(state: AppState) -> Router {
    let public = Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(auth::login))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/account", patch(auth::update_account))
        .route("/users", get(auth::list_users).post(auth::create_user))
        .route(
            "/users/{id}",
            patch(auth::update_user).delete(auth::delete_user),
        )
        .route("/stats", get(stats))
        .route("/libraries", get(list_libraries).post(create_library))
        .route(
            "/libraries/{id}",
            get(get_library).delete(delete_library).patch(update_library),
        )
        .route("/libraries/{id}/sync", post(sync_library))
        .route("/libraries/{id}/cancel", post(cancel_library_job))
        .route("/libraries/{id}/upload", post(upload_components))
        .route("/libraries/{id}/fetch", post(fetch_components))
        .route("/libraries/{id}/components", get(list_components))
        .route("/ai/status", get(ai_status))
        .route("/settings/llm", get(get_llm_settings).put(update_llm_settings))
        .route("/settings/sync", get(get_sync_settings).put(update_sync_settings))
        .route("/settings/auth", get(get_auth_settings).put(update_auth_settings))
        .route("/admin/storage/stats", get(storage_stats))
        .route("/admin/storage/cleanup", post(storage_cleanup))
        .route(
            "/admin/storage/schedule",
            get(get_cleanup_schedule).put(update_cleanup_schedule),
        )
        .route("/admin/memory", get(memory_report))
        .route("/components/{id}", get(get_component))
        .route("/components/{id}/source", get(get_component_source))
        .route("/components/{id}/docs", get(get_component_docs))
        .route("/components/{id}/examples", get(get_component_examples))
        .route("/search/components", get(search_components))
        .route("/mcp/tools", get(list_mcp_tools))
        .route("/mcp/call", post(call_mcp_tool))
        .route("/mcp/activity", get(list_mcp_activity).delete(clear_mcp_activity))
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}", get(get_task))
        .route("/api-keys", get(list_api_keys).post(create_api_key))
        .route("/api-keys/{id}", delete(delete_api_key))
        .route("/api-keys/{id}/regenerate", post(regenerate_api_key))
        .route("/logs", get(list_logs))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    public.merge(protected)
}

async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(token) = bearer {
        if let Some(user) = state.verify_session(token).await {
            request.extensions_mut().insert(AuthUser {
                user: Some(user),
                token: Some(token.to_string()),
                via_api_key: false,
            });
            return Ok(next.run(request).await);
        }
    }

    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok());

    if let Some(key) = api_key {
        let valid = state.verify_key(key).await
            || state
                .config
                .admin_api_key
                .as_deref()
                .is_some_and(|admin| admin == key);
        if valid {
            request.extensions_mut().insert(AuthUser {
                user: None,
                token: None,
                via_api_key: true,
            });
            return Ok(next.run(request).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "service": "CompiraMCP" }))
}

async fn stats(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.stats() {
        Ok(s) => Json(s).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn list_libraries(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_libraries() {
        Ok(items) => Json(items).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct CreateLibraryReq {
    name: String,
    #[serde(default)]
    repo_url: String,
    #[serde(default = "default_branch")]
    branch: String,
    rules: Option<String>,
    /// git | upload | fetch
    #[serde(default = "default_source_type")]
    source_type: String,
}

fn default_branch() -> String {
    "main".into()
}

fn default_source_type() -> String {
    "git".into()
}

async fn create_library(
    State(state): State<AppState>,
    Json(req): Json<CreateLibraryReq>,
) -> impl IntoResponse {
    let source_type = match req.source_type.as_str() {
        "upload" | "fetch" | "git" => req.source_type.as_str(),
        _ => "git",
    };

    if source_type == "git" && req.repo_url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "repo_url required for git libraries" })),
        )
            .into_response();
    }

    let id = uuid::Uuid::new_v4().to_string();
    let local_path = library_local_path(&state.config.repos_dir, &id);
    let _ = std::fs::create_dir_all(&local_path);

    let repo_url = if req.repo_url.trim().is_empty() {
        format!("{source_type}://{id}")
    } else {
        req.repo_url.clone()
    };

    match state.db.create_library_with_id(
        &id,
        &req.name,
        &repo_url,
        &req.branch,
        &local_path.to_string_lossy(),
        req.rules.as_deref(),
        source_type,
    ) {
        Ok(lib) => {
            let _ = state.db.log(
                "info",
                &format!("Library '{}' created (source={source_type})", lib.name),
                Some(&lib.id),
            );
            (StatusCode::CREATED, Json(lib)).into_response()
        }
        Err(e) => err_response(e),
    }
}

async fn ai_status(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.get_llm_settings_public(state.config.as_ref()) {
        Ok(s) => Json(serde_json::json!({
            "enabled": s.enabled,
            "model": s.model,
            "base_url": s.base_url,
            "api_key_set": s.api_key_set,
            "api_key_masked": s.api_key_masked,
        }))
        .into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_llm_settings(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.db.get_llm_settings_public(state.config.as_ref()) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct UpdateLlmSettingsReq {
    /// New API key; omit or empty to keep existing. Set clear_api_key to remove.
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

async fn update_llm_settings(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<UpdateLlmSettingsReq>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    if let Err(e) = state.db.update_llm_settings(
        req.api_key.as_deref(),
        req.base_url.as_deref(),
        req.model.as_deref(),
        req.clear_api_key,
    ) {
        return err_response(e);
    }

    let _ = state.db.log("info", "Admin updated LLM settings", None);

    match state.db.get_llm_settings_public(state.config.as_ref()) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_sync_settings(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.db.get_sync_settings(state.config.as_ref()) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct UpdateSyncSettingsReq {
    pub max_jobs: Option<usize>,
    pub parse_concurrency: Option<usize>,
    pub ingest_batch_size: Option<usize>,
    pub download_concurrency: Option<usize>,
    pub fetch_max_attempts: Option<usize>,
}

async fn update_sync_settings(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<UpdateSyncSettingsReq>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Err(e) = state.db.update_sync_settings(
        req.max_jobs,
        req.parse_concurrency,
        req.ingest_batch_size,
        req.download_concurrency,
        req.fetch_max_attempts,
    ) {
        return err_response(e);
    }
    match state.db.get_sync_settings(state.config.as_ref()) {
        Ok(s) => {
            state.tasks.apply_sync_settings(s.max_jobs);
            let _ = state.db.log(
                "info",
                &format!(
                    "Admin updated sync settings: max_jobs={}, parse={}, batch={}, download={}, fetch_attempts={}",
                    s.max_jobs, s.parse_concurrency, s.ingest_batch_size, s.download_concurrency, s.fetch_max_attempts
                ),
                None,
            );
            Json(s).into_response()
        }
        Err(e) => err_response(e),
    }
}

async fn get_auth_settings(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.db.get_auth_settings(state.config.as_ref()) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct UpdateAuthSettingsReq {
    pub session_ttl_hours: Option<u64>,
    pub remember_me_ttl_hours: Option<u64>,
    pub sliding: Option<bool>,
}

async fn update_auth_settings(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<UpdateAuthSettingsReq>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.db.update_auth_settings(
        req.session_ttl_hours,
        req.remember_me_ttl_hours,
        req.sliding,
        state.config.as_ref(),
    ) {
        Ok(s) => {
            let _ = state.db.log(
                "info",
                &format!(
                    "Admin updated auth settings: ttl={}h, remember={}h, sliding={}",
                    s.session_ttl_hours, s.remember_me_ttl_hours, s.sliding
                ),
                None,
            );
            Json(s).into_response()
        }
        Err(e) => err_response(e),
    }
}

async fn storage_stats(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match tokio::task::spawn_blocking({
        let db = state.db.clone();
        let config = state.config.clone();
        move || crate::storage::collect_storage_stats(&db, config.as_ref())
    })
    .await
    {
        Ok(Ok(s)) => Json(s).into_response(),
        Ok(Err(e)) => err_response(e),
        Err(e) => err_response(anyhow::anyhow!("storage stats join: {e}")),
    }
}

#[derive(Deserialize)]
struct CleanupReq {
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_true")]
    pub orphan_repos: bool,
    pub sync_tasks_older_than_days: Option<u64>,
    pub app_logs_older_than_days: Option<u64>,
    #[serde(default = "default_true")]
    pub expired_sessions: bool,
    #[serde(default = "default_true")]
    pub wal_checkpoint: bool,
    #[serde(default)]
    pub vacuum: bool,
}

fn default_true() -> bool {
    true
}

async fn storage_cleanup(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CleanupReq>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let opts = crate::storage::CleanupOptions {
        dry_run: req.dry_run,
        orphan_repos: req.orphan_repos,
        sync_tasks_older_than_days: req.sync_tasks_older_than_days.or(Some(30)),
        app_logs_older_than_days: req.app_logs_older_than_days.or(Some(14)),
        expired_sessions: req.expired_sessions,
        wal_checkpoint: req.wal_checkpoint,
        vacuum: req.vacuum,
    };
    match tokio::task::spawn_blocking({
        let db = state.db.clone();
        let config = state.config.clone();
        move || crate::storage::run_cleanup(&db, config.as_ref(), opts)
    })
    .await
    {
        Ok(Ok(r)) => {
            if !r.dry_run {
                let _ = state.db.record_cleanup_run(
                    &crate::storage::now_rfc3339(),
                    &crate::storage::format_cleanup_summary(&r),
                );
                let _ = state.db.log(
                    "info",
                    &format!("Manual cleanup: {}", crate::storage::format_cleanup_summary(&r)),
                    None,
                );
            }
            Json(r).into_response()
        }
        Ok(Err(e)) => err_response(e),
        Err(e) => err_response(anyhow::anyhow!("cleanup join: {e}")),
    }
}

async fn get_cleanup_schedule(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.db.get_cleanup_schedule() {
        Ok(s) => Json(s).into_response(),
        Err(e) => err_response(e),
    }
}

async fn update_cleanup_schedule(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<crate::db::CleanupSchedule>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.db.update_cleanup_schedule(&req) {
        Ok(s) => {
            let _ = state.db.log(
                "info",
                &format!(
                    "Cleanup schedule updated: enabled={}, every {}h",
                    s.enabled, s.interval_hours
                ),
                None,
            );
            Json(s).into_response()
        }
        Err(e) => err_response(e),
    }
}

async fn memory_report(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    Json(crate::storage::collect_memory_report(
        &state.db,
        state.tasks.as_ref(),
    ))
    .into_response()
}

#[derive(Deserialize)]
struct FetchComponentsReq {
    /// Natural language description of components to fetch
    prompt: String,
    /// When true, rename library from AI plan.library_name
    #[serde(default)]
    auto_name: bool,
}

async fn fetch_components(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<FetchComponentsReq>,
) -> impl IntoResponse {
    if req.prompt.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "prompt is required" })),
        )
            .into_response();
    }

    let whole_repo = crate::ai::detect_repo_intent(&req.prompt)
        .map(|(_, _, whole)| whole)
        .unwrap_or(false);
    if !whole_repo
        && !state
            .db
            .get_llm_settings(state.config.as_ref())
            .map(|s| s.enabled())
            .unwrap_or(false)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "AI fetch requires LLM API key — configure in 系统设置（仅粘贴仓库地址整库导入时可不用 LLM）"
            })),
        )
            .into_response();
    }

    match state.tasks.spawn_fetch(id, req.prompt, req.auto_name) {
        Ok(task_id) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "task_id": task_id, "auto_name": req.auto_name })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

async fn upload_components(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    let Some(lib) = (match state.db.get_library(&id) {
        Ok(l) => l,
        Err(e) => return err_response(e),
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let root = std::path::PathBuf::from(&lib.local_path);
    if let Err(e) = crate::indexer::ensure_local_dir(&root) {
        return err_response(e);
    }

    let mut auto_name = false;
    let mut relative_paths: Vec<String> = Vec::new();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        };

        let name = field.name().unwrap_or("").to_string();
        if name == "use_ai" {
            // Legacy field ignored: per-file AI enrich removed.
            let _ = field.text().await;
            continue;
        }
        if name == "auto_name" {
            let text = field.text().await.unwrap_or_default();
            auto_name = text == "1" || text.eq_ignore_ascii_case("true");
            continue;
        }

        if name != "files" && name != "file" {
            continue;
        }

        let filename = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "component.vue".into());
        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        };

        // Keep basename under uploads/ to avoid path tricks from client filenames
        let safe_name = std::path::Path::new(&filename)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("component.vue");

        if crate::indexer::is_zip_filename(safe_name) {
            match crate::indexer::extract_zip_upload(&root, safe_name, &bytes) {
                Ok(paths) => relative_paths.extend(paths),
                Err(e) => return err_response(e),
            }
            continue;
        }

        let rel = format!("uploads/{safe_name}");
        if let Err(e) = crate::indexer::write_upload_file(&root, &rel, &bytes) {
            return err_response(e);
        }
        relative_paths.push(rel);
    }

    if relative_paths.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no files uploaded (field name: files)" })),
        )
            .into_response();
    }

    match state
        .tasks
        .spawn_ingest(id, relative_paths.clone(), auto_name)
    {
        Ok(task_id) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "task_id": task_id,
                "files": relative_paths,
                "auto_name": auto_name,
            })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_library(&id) {
        Ok(Some(lib)) => Json(lib).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct UpdateLibraryReq {
    name: Option<String>,
    branch: Option<String>,
    rules: Option<String>,
}

async fn update_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateLibraryReq>,
) -> impl IntoResponse {
    let Some(mut lib) = (match state.db.get_library(&id) {
        Ok(l) => l,
        Err(e) => return err_response(e),
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if let Some(name) = req.name {
        lib.name = name;
    }
    if let Some(branch) = req.branch {
        lib.branch = branch;
    }
    if let Some(rules) = req.rules {
        lib.rules = Some(rules);
    }

    if let Err(e) = state.db.update_library(&lib) {
        return err_response(e);
    }
    Json(lib).into_response()
}

async fn delete_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Ok(Some(lib)) = state.db.get_library(&id) {
        let path = std::path::PathBuf::from(&lib.local_path);
        let _ = crate::git::remove_repo(&path);
    }
    match state.db.delete_library(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

async fn sync_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.tasks.spawn_sync(id) {
        Ok(task_id) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "task_id": task_id })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

async fn cancel_library_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.tasks.cancel_library(&id) {
        Ok(body) => Json(body).into_response(),
        Err(e) => err_response(e),
    }
}

async fn list_components(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ListComponentsQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(500).clamp(1, 10_000);
    let offset = q.offset.unwrap_or(0).max(0);
    match state
        .db
        .list_components_by_library_page(&id, limit, offset)
    {
        Ok((items, total)) => Json(serde_json::json!({
            "items": items,
            "total": total,
            "limit": limit,
            "offset": offset,
        }))
        .into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct ListComponentsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn get_component(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_component(&id) {
        Ok(Some(c)) => Json(c).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_component_source(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_component_source(&id) {
        Ok(Some(source)) => Json(serde_json::json!({ "source": source })).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_component_docs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_component_docs(&id) {
        Ok(Some(docs)) => Json(serde_json::json!({ "docs": docs })).into_response(),
        Ok(None) => Json(serde_json::json!({ "docs": "" })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_component_examples(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_component_examples(&id) {
        Ok(examples) => Json(examples).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    library_id: Option<String>,
    limit: Option<i64>,
}

async fn search_components(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(20);
    match state.db.search_components(&q.q, q.library_id.as_deref(), limit) {
        Ok(results) => Json(results).into_response(),
        Err(e) => err_response(e),
    }
}

async fn list_mcp_tools() -> impl IntoResponse {
    Json(crate::tools_exec::tool_schemas())
}

#[derive(Deserialize)]
struct McpCallReq {
    tool: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

async fn call_mcp_tool(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<McpCallReq>,
) -> impl IntoResponse {
    use std::time::Instant;
    use crate::mcp::activity::McpCaller;

    let caller = McpCaller::playground(
        auth.user.as_ref().map(|u| u.id.clone()),
        auth.user.as_ref().map(|u| u.username.clone()),
    );
    let call_id = state
        .mcp_activity
        .begin(&req.tool, &req.arguments, &caller);
    let started = Instant::now();

    match crate::tools_exec::execute_tool(&state.db, &req.tool, req.arguments).await {
        Ok(result) => {
            state.mcp_activity.finish(&call_id, true, None, started);
            Json(serde_json::json!({ "ok": true, "result": result })).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            state
                .mcp_activity
                .finish(&call_id, false, Some(msg.clone()), started);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": msg })),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
struct ActivityQuery {
    #[serde(default = "default_activity_limit")]
    limit: usize,
}

fn default_activity_limit() -> usize {
    100
}

async fn list_mcp_activity(
    _auth: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<ActivityQuery>,
) -> impl IntoResponse {
    Json(state.mcp_activity.list(q.limit)).into_response()
}

async fn clear_mcp_activity(auth: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    if !auth.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    state.mcp_activity.clear();
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
struct TaskQuery {
    library_id: Option<String>,
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(q): Query<TaskQuery>,
) -> impl IntoResponse {
    match state.db.list_sync_tasks(q.library_id.as_deref()) {
        Ok(items) => Json(items).into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_sync_task(&id) {
        Ok(Some(t)) => Json(t).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err_response(e),
    }
}

async fn list_api_keys(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_api_keys() {
        Ok(items) => Json(items).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct CreateApiKeyReq {
    name: String,
}

#[derive(Serialize)]
struct CreateApiKeyResp {
    key: crate::db::ApiKey,
}

async fn create_api_key(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyReq>,
) -> impl IntoResponse {
    let raw_key = generate_api_key();
    match state.db.create_api_key(&req.name, &raw_key) {
        Ok(k) => (StatusCode::CREATED, Json(CreateApiKeyResp { key: k })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn regenerate_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let raw_key = generate_api_key();
    match state.db.regenerate_api_key(&id, &raw_key) {
        Ok(k) => Json(CreateApiKeyResp { key: k }).into_response(),
        Err(e) => err_response(e),
    }
}

async fn delete_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_api_key(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct LogQuery {
    limit: Option<i64>,
}

async fn list_logs(
    State(state): State<AppState>,
    Query(q): Query<LogQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100);
    match state.db.list_logs(limit) {
        Ok(items) => Json(items).into_response(),
        Err(e) => err_response(e),
    }
}

fn err_response(e: anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}
