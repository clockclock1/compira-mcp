use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub data_dir: PathBuf,
    pub repos_dir: PathBuf,
    pub admin_api_key: Option<String>,
    pub admin_username: String,
    pub admin_password: Option<String>,
    /// OpenAI-compatible API key for AI ingest / enrich
    pub llm_api_key: Option<String>,
    pub llm_base_url: String,
    pub llm_model: String,
    /// Max concurrent library jobs (sync / ingest / fetch). Extra jobs wait in queue.
    pub max_jobs: usize,
    /// Components written per SQLite transaction during bulk ingest.
    pub ingest_batch_size: usize,
    /// Max parallel HTTP downloads during AI fetch.
    pub download_concurrency: usize,
    /// Rayon threads used when parsing component files.
    pub parse_concurrency: usize,
    /// Default session TTL in hours (non–remember-me).
    pub session_ttl_hours: u64,
    /// Session TTL when "remember me" is checked.
    pub remember_me_ttl_hours: u64,
    /// Extend session expiry on activity.
    pub session_sliding: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = resolve_data_dir();
        let _ = std::fs::create_dir_all(&data_dir);

        let database_url = std::env::var("COMPIRA_DATABASE_URL")
            .unwrap_or_else(|_| data_dir.join("compira.db").to_string_lossy().into());

        let repos_dir = data_dir.join("repos");

        Self {
            host: std::env::var("COMPIRA_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("COMPIRA_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url,
            data_dir,
            repos_dir,
            admin_api_key: std::env::var("COMPIRA_ADMIN_API_KEY").ok(),
            admin_username: std::env::var("COMPIRA_ADMIN_USERNAME")
                .unwrap_or_else(|_| "admin".into()),
            admin_password: std::env::var("COMPIRA_ADMIN_PASSWORD").ok(),
            llm_api_key: std::env::var("COMPIRA_LLM_API_KEY")
                .ok()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
            llm_base_url: std::env::var("COMPIRA_LLM_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
            llm_model: std::env::var("COMPIRA_LLM_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".into()),
            max_jobs: env_usize("COMPIRA_MAX_JOBS", 3).clamp(1, 64),
            ingest_batch_size: env_usize("COMPIRA_INGEST_BATCH_SIZE", 250).clamp(20, 5000),
            download_concurrency: env_usize("COMPIRA_DOWNLOAD_CONCURRENCY", 8).clamp(1, 64),
            parse_concurrency: env_usize("COMPIRA_PARSE_CONCURRENCY", 0), // 0 = num_cpus
            session_ttl_hours: env_u64("COMPIRA_SESSION_TTL_HOURS", 8).clamp(1, 24 * 90),
            remember_me_ttl_hours: env_u64("COMPIRA_REMEMBER_TTL_HOURS", 168).clamp(1, 24 * 365),
            session_sliding: env_bool("COMPIRA_SESSION_SLIDING", true),
        }
    }

    pub fn llm_enabled(&self) -> bool {
        self.llm_api_key
            .as_ref()
            .is_some_and(|k| !k.trim().is_empty())
    }
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => default,
    }
}

/// Resolve data dir to an absolute path. Avoid `canonicalize` on Windows so we
/// don't introduce `\\?\` prefixes that break libgit2.
fn resolve_data_dir() -> PathBuf {
    let raw = std::env::var("COMPIRA_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data"));

    let absolute = if raw.is_absolute() {
        raw
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(raw)
    };

    #[cfg(windows)]
    {
        let s = absolute.to_string_lossy();
        if s.chars().any(|c| !c.is_ascii()) {
            tracing::warn!(
                "COMPIRA_DATA_DIR contains non-ASCII characters ({s}). \
                 libgit2 on Windows may fail cloning repos; set COMPIRA_DATA_DIR to an ASCII path \
                 (e.g. C:\\compira-data)."
            );
        }
    }

    absolute
}
