use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, Notify};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::db::Database;
use crate::indexer::{self, is_cancelled};

pub fn library_local_path(repos_dir: &std::path::Path, library_id: &str) -> PathBuf {
    repos_dir.join(library_id)
}

/// Limits concurrent library jobs; max can be changed at runtime from settings.
struct DynamicLimiter {
    active: AtomicUsize,
    max: AtomicUsize,
    notify: Notify,
}

impl DynamicLimiter {
    fn new(max: usize) -> Self {
        Self {
            active: AtomicUsize::new(0),
            max: AtomicUsize::new(max.clamp(1, 64)),
            notify: Notify::new(),
        }
    }

    fn set_max(&self, max: usize) {
        self.max.store(max.clamp(1, 64), Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    fn max(&self) -> usize {
        self.max.load(Ordering::Relaxed)
    }

    async fn acquire(&self) {
        loop {
            let max = self.max.load(Ordering::Acquire);
            let cur = self.active.load(Ordering::Acquire);
            if cur < max {
                if self
                    .active
                    .compare_exchange(cur, cur + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return;
                }
                continue;
            }
            self.notify.notified().await;
        }
    }

    fn release(&self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
        self.notify.notify_one();
    }
}

#[derive(Clone)]
pub struct TaskManager {
    db: Database,
    config: Arc<Config>,
    tx: broadcast::Sender<String>,
    limiter: Arc<DynamicLimiter>,
    /// Active cancel tokens keyed by library id.
    cancels: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl TaskManager {
    pub fn new(db: Database, config: Config) -> Self {
        let (tx, _) = broadcast::channel(256);
        let max_jobs = db
            .get_sync_settings(&config)
            .map(|s| s.max_jobs)
            .unwrap_or(config.max_jobs);
        tracing::info!("Task manager: max concurrent library jobs = {max_jobs} (adds unlimited)");
        Self {
            db,
            limiter: Arc::new(DynamicLimiter::new(max_jobs)),
            config: Arc::new(config),
            tx,
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Apply admin-configured sync concurrency (takes effect for newly waiting jobs).
    pub fn apply_sync_settings(&self, max_jobs: usize) {
        tracing::info!("Updating max concurrent library jobs -> {max_jobs}");
        self.limiter.set_max(max_jobs);
    }

    pub fn max_jobs(&self) -> usize {
        self.limiter.max()
    }

    pub fn active_jobs(&self) -> usize {
        self.limiter.active.load(Ordering::Relaxed)
    }

    fn register_cancel(&self, library_id: &str) -> CancellationToken {
        let mut map = self.cancels.lock().unwrap();
        if let Some(prev) = map.remove(library_id) {
            prev.cancel();
        }
        let token = CancellationToken::new();
        map.insert(library_id.to_string(), token.clone());
        token
    }

    /// Cancel pending/running jobs for a library. Already-indexed components are kept.
    pub fn cancel_library(&self, library_id: &str) -> anyhow::Result<serde_json::Value> {
        {
            let mut map = self.cancels.lock().unwrap();
            if let Some(token) = map.remove(library_id) {
                token.cancel();
            }
        }
        let marked = self.db.cancel_library_tasks(library_id)?;
        let count = self.db.count_library_components(library_id).unwrap_or(0);
        // Keep partial index visible as ready (or leave syncing→ready)
        let _ = self
            .db
            .update_library_status(library_id, "ready", Some(count));
        let _ = self.db.log(
            "info",
            &format!("Library job cancelled by user (kept {count} components)"),
            Some(library_id),
        );
        Ok(serde_json::json!({
            "cancelled": true,
            "tasks_marked": marked,
            "component_count": count,
        }))
    }

    async fn run_job<F, Fut>(
        &self,
        task_id: String,
        library_id: String,
        cancel: CancellationToken,
        work: F,
    ) where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
    {
        let limiter = self.limiter.clone();
        let db = self.db.clone();
        let tx = self.tx.clone();
        let cancels = self.cancels.clone();
        let _ = tx.send(task_id.clone());

        let _ = db.update_sync_task(
            &task_id,
            0,
            &format!("Waiting for sync slot (max {})...", limiter.max()),
            Some("pending"),
        );

        if cancel.is_cancelled() {
            let _ = db.update_sync_task(&task_id, 0, "Cancelled before start", Some("cancelled"));
            Self::cleanup_token(&cancels, &library_id, &cancel);
            return;
        }

        limiter.acquire().await;

        if cancel.is_cancelled() {
            limiter.release();
            let _ = db.update_sync_task(&task_id, 0, "Cancelled while queued", Some("cancelled"));
            let count = db.count_library_components(&library_id).unwrap_or(0);
            let _ = db.update_library_status(&library_id, "ready", Some(count));
            Self::cleanup_token(&cancels, &library_id, &cancel);
            return;
        }

        let _ = db.update_sync_task(&task_id, 1, "Queued job started...", Some("running"));
        let result = work(cancel.clone()).await;
        limiter.release();
        Self::cleanup_token(&cancels, &library_id, &cancel);

        match result {
            Ok(()) => {}
            Err(e) if is_cancelled(&e) || cancel.is_cancelled() => {
                let count = db.count_library_components(&library_id).unwrap_or(0);
                let msg = format!("已中断，保留已入库 {count} 个组件");
                let _ = db.update_sync_task(&task_id, 100, &msg, Some("cancelled"));
                let _ = db.update_library_status(&library_id, "ready", Some(count));
                let _ = db.log("info", &msg, Some(&library_id));
            }
            Err(e) => {
                let msg = format!("Job failed: {e}");
                let _ = db.update_sync_task(&task_id, 0, &msg, Some("failed"));
                let _ = db.set_library_error(&library_id, &msg);
                let _ = db.log("error", &msg, Some(&library_id));
            }
        }
    }

    fn cleanup_token(
        cancels: &Mutex<HashMap<String, CancellationToken>>,
        library_id: &str,
        token: &CancellationToken,
    ) {
        let mut map = cancels.lock().unwrap();
        if map.get(library_id).is_some_and(|t| t == token) {
            map.remove(library_id);
        }
    }

    pub fn spawn_sync(&self, library_id: String) -> anyhow::Result<String> {
        let library = self
            .db
            .get_library(&library_id)?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        let task = self.db.create_sync_task(&library_id)?;
        let task_id = task.id.clone();
        let return_task_id = task_id.clone();
        let cancel = self.register_cancel(&library_id);

        let db = self.db.clone();
        let config = self.config.clone();
        let lib_id = library_id.clone();
        let repo_url = library.repo_url.clone();
        let branch = library.branch.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();
        let source_type = library.source_type.clone();
        let this = self.clone();

        tokio::spawn(async move {
            this.run_job(task_id.clone(), lib_id.clone(), cancel, move |cancel| async move {
                indexer::sync_or_reindex(
                    &db,
                    config.as_ref(),
                    &task_id,
                    &lib_id,
                    &source_type,
                    &repo_url,
                    &branch,
                    &local_path,
                    &name,
                    &cancel,
                )
                .await
                .map(|_| ())
            })
            .await;
        });

        Ok(return_task_id)
    }

    pub fn spawn_ingest(
        &self,
        library_id: String,
        relative_paths: Vec<String>,
        auto_name: bool,
    ) -> anyhow::Result<String> {
        let library = self
            .db
            .get_library(&library_id)?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        let task = self.db.create_sync_task(&library_id)?;
        let task_id = task.id.clone();
        let return_task_id = task_id.clone();
        let cancel = self.register_cancel(&library_id);

        let db = self.db.clone();
        let config = self.config.clone();
        let lib_id = library_id.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();
        let this = self.clone();

        tokio::spawn(async move {
            this.run_job(task_id.clone(), lib_id.clone(), cancel, move |cancel| async move {
                indexer::ingest_files(
                    &db,
                    Some(config.as_ref()),
                    &task_id,
                    &lib_id,
                    &local_path,
                    &name,
                    &relative_paths,
                    auto_name,
                    &cancel,
                )
                .await
                .map(|_| ())
            })
            .await;
        });

        Ok(return_task_id)
    }

    pub fn spawn_fetch(
        &self,
        library_id: String,
        prompt: String,
        auto_name: bool,
    ) -> anyhow::Result<String> {
        let library = self
            .db
            .get_library(&library_id)?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        if prompt.trim().is_empty() {
            anyhow::bail!("prompt is required");
        }
        let whole_repo = crate::ai::detect_repo_intent(&prompt)
            .map(|(_, _, whole)| whole)
            .unwrap_or(false);
        if !whole_repo && !self.db.get_llm_settings(self.config.as_ref())?.enabled() {
            anyhow::bail!("AI fetch requires LLM API key (configure in admin settings)");
        }

        let task = self.db.create_sync_task(&library_id)?;
        let task_id = task.id.clone();
        let return_task_id = task_id.clone();
        let cancel = self.register_cancel(&library_id);

        let db = self.db.clone();
        let config = self.config.clone();
        let lib_id = library_id.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();
        let this = self.clone();

        tokio::spawn(async move {
            this.run_job(task_id.clone(), lib_id.clone(), cancel, move |cancel| async move {
                indexer::fetch_and_ingest(
                    &db,
                    config.as_ref(),
                    &task_id,
                    &lib_id,
                    &local_path,
                    &name,
                    &prompt,
                    auto_name,
                    &cancel,
                )
                .await
                .map(|_| ())
            })
            .await;
        });

        Ok(return_task_id)
    }
}
