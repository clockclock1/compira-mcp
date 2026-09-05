use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::Config;
use crate::db::Database;
use crate::indexer;

#[derive(Clone)]
pub struct TaskManager {
    db: Database,
    config: Arc<Config>,
    tx: broadcast::Sender<String>,
}

impl TaskManager {
    pub fn new(db: Database, config: Config) -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            db,
            config: Arc::new(config),
            tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn spawn_sync(&self, library_id: String) -> anyhow::Result<String> {
        let library = self
            .db
            .get_library(&library_id)?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        if library.source_type != "git" {
            anyhow::bail!("Library source_type is '{}', use upload/fetch instead of git sync", library.source_type);
        }

        let task = self.db.create_sync_task(&library_id)?;
        let task_id = task.id.clone();
        let return_task_id = task_id.clone();

        let db = self.db.clone();
        let tx = self.tx.clone();
        let lib_id = library_id.clone();
        let repo_url = library.repo_url.clone();
        let branch = library.branch.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();

        tokio::spawn(async move {
            let _ = tx.send(task_id.clone());
            let result = indexer::sync_library(
                &db,
                &task_id,
                &lib_id,
                &repo_url,
                &branch,
                &local_path,
                &name,
            )
            .await;

            if let Err(e) = result {
                let msg = format!("Sync failed: {e}");
                let _ = db.update_sync_task(&task_id, 0, &msg, Some("failed"));
                let _ = db.set_library_error(&lib_id, &msg);
                let _ = db.log("error", &msg, Some(&lib_id));
            }
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
        let tx = self.tx.clone();
        let lib_id = library_id.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();

        tokio::spawn(async move {
            let _ = tx.send(task_id.clone());
            let result = indexer::ingest_files(
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
            .await;

            if let Err(e) = result {
                let msg = format!("Ingest failed: {e}");
                let _ = db.update_sync_task(&task_id, 0, &msg, Some("failed"));
                let _ = db.set_library_error(&lib_id, &msg);
                let _ = db.log("error", &msg, Some(&lib_id));
            }
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
        let tx = self.tx.clone();
        let lib_id = library_id.clone();
        let local_path = PathBuf::from(&library.local_path);
        let name = library.name.clone();

        tokio::spawn(async move {
            let _ = tx.send(task_id.clone());
            let result = indexer::fetch_and_ingest(
                &db,
                config.as_ref(),
                &task_id,
                &lib_id,
                &local_path,
                &name,
                &prompt,
                auto_name,
            )
            .await;

            if let Err(e) = result {
                let msg = format!("AI fetch failed: {e}");
                let _ = db.update_sync_task(&task_id, 0, &msg, Some("failed"));
                let _ = db.set_library_error(&lib_id, &msg);
                let _ = db.log("error", &msg, Some(&lib_id));
            }
        });

        Ok(return_task_id)
    }
}

pub fn library_local_path(repos_dir: &std::path::Path, library_id: &str) -> PathBuf {
    repos_dir.join(library_id)
}
