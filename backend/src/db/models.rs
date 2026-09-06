use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: String,
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub status: String,
    pub component_count: i64,
    pub rules: Option<String>,
    /// git | upload | fetch
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// Last sync/ingest/fetch failure message (cleared on success)
    #[serde(default)]
    pub last_error: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

fn default_source_type() -> String {
    "git".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub library_id: String,
    pub name: String,
    pub file_path: String,
    pub framework: String,
    pub description: Option<String>,
    pub props: Vec<PropDef>,
    pub events: Vec<EventDef>,
    pub slots: Vec<SlotDef>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropDef {
    pub name: String,
    pub r#type: Option<String>,
    pub default: Option<String>,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventDef {
    pub name: String,
    pub payload: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlotDef {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentExample {
    pub id: String,
    pub component_id: String,
    pub title: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub role: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    /// Full secret for admin UI (always available for keys created after this feature).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTask {
    pub id: String,
    pub library_id: String,
    pub status: String,
    pub progress: i32,
    pub message: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: i64,
    pub level: String,
    pub message: String,
    pub context: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSearchResult {
    pub id: String,
    pub library_id: String,
    pub library_name: String,
    pub name: String,
    pub file_path: String,
    pub framework: String,
    pub description: Option<String>,
    pub score: f64,
}

/// Runtime LLM config (resolved from DB, optionally seeded by env).
#[derive(Debug, Clone)]
pub struct LlmSettings {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
}

impl LlmSettings {
    pub fn enabled(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.trim().is_empty())
    }

    pub fn mask_key(&self) -> Option<String> {
        let key = self.api_key.as_ref()?.trim();
        if key.is_empty() {
            return None;
        }
        if key.len() <= 8 {
            return Some("••••••••".into());
        }
        Some(format!("{}••••{}", &key[..4], &key[key.len() - 4..]))
    }
}

/// Public view for admin UI (never returns full API key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettingsPublic {
    pub enabled: bool,
    pub api_key_set: bool,
    pub api_key_masked: Option<String>,
    pub base_url: String,
    pub model: String,
}

/// Sync / ingest concurrency settings (admin configurable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSettings {
    /// Max libraries syncing/ingesting in parallel (adds are unlimited).
    pub max_jobs: usize,
    /// Parallel file parse threads (0 = auto / CPU count).
    pub parse_concurrency: usize,
    /// Components per DB write transaction.
    pub ingest_batch_size: usize,
    /// Parallel HTTP downloads for AI fetch.
    pub download_concurrency: usize,
}
