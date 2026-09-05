use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS libraries (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            repo_url TEXT NOT NULL,
            branch TEXT NOT NULL DEFAULT 'main',
            local_path TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            component_count INTEGER NOT NULL DEFAULT 0,
            rules TEXT,
            last_synced_at TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS components (
            id TEXT PRIMARY KEY,
            library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            framework TEXT NOT NULL DEFAULT 'vue',
            description TEXT,
            props_json TEXT NOT NULL DEFAULT '[]',
            events_json TEXT NOT NULL DEFAULT '[]',
            slots_json TEXT NOT NULL DEFAULT '[]',
            tags_json TEXT NOT NULL DEFAULT '[]',
            source_content TEXT,
            UNIQUE(library_id, file_path)
        );

        CREATE TABLE IF NOT EXISTS component_docs (
            component_id TEXT PRIMARY KEY REFERENCES components(id) ON DELETE CASCADE,
            content TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS component_examples (
            id TEXT PRIMARY KEY,
            component_id TEXT NOT NULL REFERENCES components(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            code TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL UNIQUE,
            key_prefix TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_used_at TEXT
        );

        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE COLLATE NOCASE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'user',
            display_name TEXT,
            created_at TEXT NOT NULL,
            last_login_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            token_hash TEXT NOT NULL UNIQUE,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

        CREATE TABLE IF NOT EXISTS sync_tasks (
            id TEXT PRIMARY KEY,
            library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
            status TEXT NOT NULL DEFAULT 'pending',
            progress INTEGER NOT NULL DEFAULT 0,
            message TEXT NOT NULL DEFAULT '',
            started_at TEXT NOT NULL,
            finished_at TEXT
        );

        CREATE TABLE IF NOT EXISTS app_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            context TEXT,
            created_at TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS components_fts USING fts5(
            component_id UNINDEXED,
            library_id UNINDEXED,
            library_name,
            name,
            description,
            props_text,
            events_text,
            tags_text,
            source_snippet,
            tokenize = 'unicode61'
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );
        ",
    )?;

    // Migrations for existing DBs
    let _ = conn.execute(
        "ALTER TABLE libraries ADD COLUMN source_type TEXT NOT NULL DEFAULT 'git'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE libraries ADD COLUMN last_error TEXT",
        [],
    );
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );",
    );

    Ok(())
}
