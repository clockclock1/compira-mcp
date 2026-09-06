use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{broadcast, Semaphore};

use crate::config::Config;
use crate::db::Database;
use crate::indexer;

#[derive(Clone)]
pub struct TaskManager {
    db: Database,
    config: Arc<Config>,
    tx: broadcast::Sender<String>,
    /// Limits how many library jobs run at once so the API stays responsive.
    job_slots: Arc<Semaphore>,
}

impl TaskManager {
    pub fn new(db: Database, config: Config) -> Self {
        let (tx, _) = broadcast::channel(256);
        let max_jobs = config.max_jobs;
        tracing::info!("Task manager: max concurrent library jobs = {max_jobs}");
        Self {
            db,
            job_slots: Arc::new(Semaphore::new(max_jobs)),
            config: Arc::new(config),
            tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    async fn run_job<F, Fut>(&self, task_id: String, library_id: String, work: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
    {
        let slots = self.job_slots.clone();
        let db = self.db.clone();
        let tx = self.tx.clone();
        let _ = tx.send(task_id.clone());

        let permit = match slots.acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                let msg = "Job queue closed".to_string();
                let _ = db.update_sync_task(&task_id, 0, &msg, Some("failed"));
                let _ = db.set_library_error(&library_id, &msg);
                return;
            }
        };

        let _ = db.update_sync_task(&task_id, 1, "Queued job started...", Some("running"));
        let result = work().await;
        drop(permit);

        if let Err(e) = result {
            let msg = format!("Job failed: {e}");
            let _ = db.update_sync_task(&task_id, 0, &msg, Some("failed"));
            let _ = db.set_library_error(&library_id, &msg);
            let _ = db.log("error", &msg, Some(&library_id));
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
            this.run_job(task_id.clone(), lib_id.clone(), move || async move {
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
        use_ai: bool,
        auto_name: bool,
    ) -> anyhow::Result<String> {
        let library = self
            .db
            .get_library(&library_id)?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        let task = self.db.create_sync_task(&library_id)?;
        let task_id = task.id.clone();
        let return_task_id = task_id.clone();

        let db = self.db.clone();
        let config = self.config.clone();
        let lib_id = library_id.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();
        let this = self.clone();

        tokio::spawn(async move {
            this.run_job(task_id.clone(), lib_id.clone(), move || async move {
                indexer::ingest_files(
                    &db,
                    Some(config.as_ref()),
                    &task_id,
                    &lib_id,
                    &local_path,
                    &name,
                    &relative_paths,
                    use_ai,
                    auto_name,
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

        let db = self.db.clone();
        let config = self.config.clone();
        let lib_id = library_id.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();
        let this = self.clone();

        tokio::spawn(async move {
            this.run_job(task_id.clone(), lib_id.clone(), move || async move {
                indexer::fetch_and_ingest(
                    &db,
                    config.as_ref(),
                    &task_id,
                    &lib_id,
                    &local_path,
                    &name,
                    &prompt,
                    auto_name,
                )
                .await
                .map(|_| ())
            })
            .await;
        });

        Ok(return_task_id)
    }
}

pub fn library_local_path(repos_dir: &std::path::Path, library_id: &str) -> PathBuf {
    repos_dir.join(library_id)
}
