mod models;
mod schema;

pub use models::*;
pub use schema::init_schema;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        // Runtime pragmas (also set in schema init; re-apply for existing DBs)
        let _ = conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 30000;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -65536;
            PRAGMA mmap_size = 268435456;
            ",
        );
        init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn log(&self, level: &str, message: &str, context: Option<&str>) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_logs (level, message, context, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![level, message, context, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn list_logs(&self, limit: i64) -> anyhow::Result<Vec<LogEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, level, message, context, created_at FROM app_logs ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(LogEntry {
                id: row.get(0)?,
                level: row.get(1)?,
                message: row.get(2)?,
                context: row.get(3)?,
                created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn create_library(
        &self,
        name: &str,
        repo_url: &str,
        branch: &str,
        local_path: &str,
        rules: Option<&str>,
        source_type: &str,
    ) -> anyhow::Result<Library> {
        self.create_library_with_id(
            &Uuid::new_v4().to_string(),
            name,
            repo_url,
            branch,
            local_path,
            rules,
            source_type,
        )
    }

    pub fn create_library_with_id(
        &self,
        id: &str,
        name: &str,
        repo_url: &str,
        branch: &str,
        local_path: &str,
        rules: Option<&str>,
        source_type: &str,
    ) -> anyhow::Result<Library> {
        let now = Utc::now();
        let source_type = if source_type.is_empty() {
            "git"
        } else {
            source_type
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO libraries (id, name, repo_url, branch, local_path, status, rules, created_at, source_type)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?8)",
            params![
                id,
                name,
                repo_url,
                branch,
                local_path,
                rules,
                now.to_rfc3339(),
                source_type
            ],
        )?;
        Ok(Library {
            id: id.into(),
            name: name.into(),
            repo_url: repo_url.into(),
            branch: branch.into(),
            local_path: local_path.into(),
            status: "pending".into(),
            component_count: 0,
            rules: rules.map(String::from),
            source_type: source_type.into(),
            last_error: None,
            last_synced_at: None,
            created_at: now,
        })
    }

    fn map_library_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Library> {
        Ok(Library {
            id: row.get(0)?,
            name: row.get(1)?,
            repo_url: row.get(2)?,
            branch: row.get(3)?,
            local_path: row.get(4)?,
            status: row.get(5)?,
            component_count: row.get(6)?,
            rules: row.get(7)?,
            last_synced_at: row
                .get::<_, Option<String>>(8)?
                .and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>(9)?.parse().unwrap_or_else(|_| Utc::now()),
            source_type: row
                .get::<_, Option<String>>(10)?
                .unwrap_or_else(|| "git".into()),
            last_error: row.get(11)?,
        })
    }

    pub fn list_libraries(&self) -> anyhow::Result<Vec<Library>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, repo_url, branch, local_path, status, component_count, rules, last_synced_at, created_at, COALESCE(source_type, 'git'), last_error
             FROM libraries ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], Self::map_library_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_library(&self, id: &str) -> anyhow::Result<Option<Library>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, repo_url, branch, local_path, status, component_count, rules, last_synced_at, created_at, COALESCE(source_type, 'git'), last_error
             FROM libraries WHERE id = ?1",
            [id],
            Self::map_library_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_component_id_by_path(
        &self,
        library_id: &str,
        file_path: &str,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id FROM components WHERE library_id = ?1 AND file_path = ?2",
            params![library_id, file_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn count_library_components(&self, library_id: &str) -> anyhow::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM components WHERE library_id = ?1",
            [library_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn update_library_status(
        &self,
        id: &str,
        status: &str,
        component_count: Option<i64>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        if let Some(count) = component_count {
            conn.execute(
                "UPDATE libraries SET status = ?1, component_count = ?2, last_synced_at = ?3, last_error = NULL WHERE id = ?4",
                params![status, count, Utc::now().to_rfc3339(), id],
            )?;
        } else if status == "ready" || status == "syncing" {
            conn.execute(
                "UPDATE libraries SET status = ?1, last_error = NULL WHERE id = ?2",
                params![status, id],
            )?;
        } else {
            conn.execute(
                "UPDATE libraries SET status = ?1 WHERE id = ?2",
                params![status, id],
            )?;
        }
        Ok(())
    }

    pub fn set_library_error(&self, id: &str, error: &str) -> anyhow::Result<()> {
        let msg: String = error.chars().take(2000).collect();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE libraries SET status = 'error', last_error = ?1 WHERE id = ?2",
            params![msg, id],
        )?;
        Ok(())
    }

    pub fn update_library(&self, lib: &Library) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE libraries SET name = ?1, branch = ?2, rules = ?3, repo_url = ?4 WHERE id = ?5",
            params![lib.name, lib.branch, lib.rules, lib.repo_url, lib.id],
        )?;
        Ok(())
    }

    pub fn update_library_name(&self, id: &str, name: &str) -> anyhow::Result<()> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("library name cannot be empty");
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE libraries SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_library(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM components_fts WHERE library_id = ?1", [id])?;
        conn.execute("DELETE FROM libraries WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn upsert_component(&self, component: &Component, source: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        Self::upsert_component_conn(&conn, component, source)
    }

    fn upsert_component_conn(
        conn: &Connection,
        component: &Component,
        source: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT INTO components (id, library_id, name, file_path, framework, description, props_json, events_json, slots_json, tags_json, source_content)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(library_id, file_path) DO UPDATE SET
               name = excluded.name,
               framework = excluded.framework,
               description = excluded.description,
               props_json = excluded.props_json,
               events_json = excluded.events_json,
               slots_json = excluded.slots_json,
               tags_json = excluded.tags_json,
               source_content = excluded.source_content",
            params![
                component.id,
                component.library_id,
                component.name,
                component.file_path,
                component.framework,
                component.description,
                serde_json::to_string(&component.props)?,
                serde_json::to_string(&component.events)?,
                serde_json::to_string(&component.slots)?,
                serde_json::to_string(&component.tags)?,
                source,
            ],
        )?;
        Ok(())
    }

    /// Bulk upsert components + docs + examples + FTS in one transaction.
    pub fn upsert_components_batch(
        &self,
        library_name: &str,
        items: &[(Component, String, String, Vec<(String, String)>)],
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Preserve stable ids for paths in this batch only (avoid full-library map)
        let library_id = &items[0].0.library_id;
        let mut existing: std::collections::HashMap<String, String> =
            std::collections::HashMap::with_capacity(items.len());
        {
            let mut stmt = tx.prepare(
                "SELECT id FROM components WHERE library_id = ?1 AND file_path = ?2",
            )?;
            for (component, _, _, _) in items {
                if let Ok(Some(id)) = stmt
                    .query_row(params![library_id, &component.file_path], |r| {
                        r.get::<_, String>(0)
                    })
                    .optional()
                {
                    existing.insert(component.file_path.clone(), id);
                }
            }
        }

        for (component, source, docs, examples) in items {
            let mut component = component.clone();
            if let Some(id) = existing.get(&component.file_path) {
                component.id = id.clone();
            }

            Self::upsert_component_conn(&tx, &component, source)?;

            if !docs.is_empty() {
                tx.execute(
                    "INSERT INTO component_docs (component_id, content) VALUES (?1, ?2)
                     ON CONFLICT(component_id) DO UPDATE SET content = excluded.content",
                    params![component.id, docs],
                )?;
            }

            tx.execute(
                "DELETE FROM component_examples WHERE component_id = ?1",
                [&component.id],
            )?;
            for (title, code) in examples {
                tx.execute(
                    "INSERT INTO component_examples (id, component_id, title, code) VALUES (?1, ?2, ?3, ?4)",
                    params![Uuid::new_v4().to_string(), component.id, title, code],
                )?;
            }

            tx.execute(
                "DELETE FROM components_fts WHERE component_id = ?1",
                [&component.id],
            )?;
            let props_text: String = component
                .props
                .iter()
                .map(|p| format!("{} {}", p.name, p.description.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(" ");
            let events_text: String = component
                .events
                .iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let tags_text = component.tags.join(" ");
            let snippet: String = source.chars().take(2000).collect();
            tx.execute(
                "INSERT INTO components_fts (component_id, library_id, library_name, name, description, props_text, events_text, tags_text, source_snippet)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    component.id,
                    component.library_id,
                    library_name,
                    component.name,
                    component.description,
                    props_text,
                    events_text,
                    tags_text,
                    snippet,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn set_component_docs(&self, component_id: &str, content: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO component_docs (component_id, content) VALUES (?1, ?2)
             ON CONFLICT(component_id) DO UPDATE SET content = excluded.content",
            params![component_id, content],
        )?;
        Ok(())
    }

    pub fn set_component_examples(
        &self,
        component_id: &str,
        examples: &[(String, String)],
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM component_examples WHERE component_id = ?1",
            [component_id],
        )?;
        for (title, code) in examples {
            conn.execute(
                "INSERT INTO component_examples (id, component_id, title, code) VALUES (?1, ?2, ?3, ?4)",
                params![Uuid::new_v4().to_string(), component_id, title, code],
            )?;
        }
        Ok(())
    }

    pub fn clear_library_components(&self, library_id: &str) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM components_fts WHERE library_id = ?1",
            [library_id],
        )?;
        tx.execute("DELETE FROM components WHERE library_id = ?1", [library_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn index_component(
        &self,
        component: &Component,
        library_name: &str,
        source_snippet: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM components_fts WHERE component_id = ?1",
            [&component.id],
        )?;
        let props_text: String = component
            .props
            .iter()
            .map(|p| format!("{} {}", p.name, p.description.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
            .join(" ");
        let events_text: String = component
            .events
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let tags_text = component.tags.join(" ");
        conn.execute(
            "INSERT INTO components_fts (component_id, library_id, library_name, name, description, props_text, events_text, tags_text, source_snippet)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                component.id,
                component.library_id,
                library_name,
                component.name,
                component.description,
                props_text,
                events_text,
                tags_text,
                source_snippet.chars().take(2000).collect::<String>(),
            ],
        )?;
        Ok(())
    }

    pub fn search_components(
        &self,
        query: &str,
        library_id: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<ComponentSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let fts_query = query
            .split_whitespace()
            .map(|w| format!("\"{}\"", w.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" OR ");

        let _sql = if library_id.is_some() {
            "SELECT component_id, library_id, library_name, name, description, bm25(components_fts) as score
             FROM components_fts WHERE components_fts MATCH ?1 AND library_id = ?2
             ORDER BY score LIMIT ?3"
        } else {
            "SELECT component_id, library_id, library_name, name, description, bm25(components_fts) as score
             FROM components_fts WHERE components_fts MATCH ?1
             ORDER BY score LIMIT ?2"
        };

        let sql = _sql;

        let mut results = Vec::new();
        if let Some(lib_id) = library_id {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![fts_query, lib_id, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, f64>(5)?,
                ))
            })?;
            for row in rows.flatten() {
                results.push(self.build_search_result(&conn, row)?);
            }
        } else {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![fts_query, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, f64>(5)?,
                ))
            })?;
            for row in rows.flatten() {
                results.push(self.build_search_result(&conn, row)?);
            }
        }
        Ok(results)
    }

    fn build_search_result(
        &self,
        conn: &Connection,
        row: (String, String, String, String, Option<String>, f64),
    ) -> anyhow::Result<ComponentSearchResult> {
        let (id, library_id, library_name, name, description, score) = row;
        let (file_path, framework): (String, String) = conn.query_row(
            "SELECT file_path, framework FROM components WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(ComponentSearchResult {
            id,
            library_id,
            library_name,
            name,
            file_path,
            framework,
            description,
            score,
        })
    }

    pub fn search_source(
        &self,
        query: &str,
        library_id: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query.replace('%', ""));
        let sql = if library_id.is_some() {
            "SELECT c.id, c.name, c.file_path, c.library_id, l.name, substr(c.source_content, 1, 500)
             FROM components c JOIN libraries l ON c.library_id = l.id
             WHERE c.source_content LIKE ?1 AND c.library_id = ?2 LIMIT ?3"
        } else {
            "SELECT c.id, c.name, c.file_path, c.library_id, l.name, substr(c.source_content, 1, 500)
             FROM components c JOIN libraries l ON c.library_id = l.id
             WHERE c.source_content LIKE ?1 LIMIT ?2"
        };

        let mut results = Vec::new();
        if let Some(lib_id) = library_id {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![pattern, lib_id, limit], |row| {
                Ok(serde_json::json!({
                    "component_id": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "file_path": row.get::<_, String>(2)?,
                    "library_id": row.get::<_, String>(3)?,
                    "library_name": row.get::<_, String>(4)?,
                    "snippet": row.get::<_, String>(5)?,
                }))
            })?;
            for r in rows.flatten() {
                results.push(r);
            }
        } else {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![pattern, limit], |row| {
                Ok(serde_json::json!({
                    "component_id": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "file_path": row.get::<_, String>(2)?,
                    "library_id": row.get::<_, String>(3)?,
                    "library_name": row.get::<_, String>(4)?,
                    "snippet": row.get::<_, String>(5)?,
                }))
            })?;
            for r in rows.flatten() {
                results.push(r);
            }
        }
        Ok(results)
    }

    pub fn get_component(&self, id: &str) -> anyhow::Result<Option<Component>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, library_id, name, file_path, framework, description, props_json, events_json, slots_json, tags_json
             FROM components WHERE id = ?1",
            [id],
            |row| {
                Ok(Component {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    name: row.get(2)?,
                    file_path: row.get(3)?,
                    framework: row.get(4)?,
                    description: row.get(5)?,
                    props: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    events: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                    slots: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                    tags: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_component_source(&self, id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT source_content FROM components WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_component_docs(&self, id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT content FROM component_docs WHERE component_id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_component_examples(&self, id: &str) -> anyhow::Result<Vec<ComponentExample>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, component_id, title, code FROM component_examples WHERE component_id = ?1",
        )?;
        let rows = stmt.query_map([id], |row| {
            Ok(ComponentExample {
                id: row.get(0)?,
                component_id: row.get(1)?,
                title: row.get(2)?,
                code: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_components_by_library(&self, library_id: &str) -> anyhow::Result<Vec<Component>> {
        self.list_components_by_library_page(library_id, 10_000, 0)
            .map(|(items, _)| items)
    }

    /// Paginated component list (no source_content). Returns (items, total).
    pub fn list_components_by_library_page(
        &self,
        library_id: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<(Vec<Component>, i64)> {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM components WHERE library_id = ?1",
            [library_id],
            |r| r.get(0),
        )?;
        let limit = limit.clamp(1, 50_000);
        let offset = offset.max(0);
        let mut stmt = conn.prepare(
            "SELECT id, library_id, name, file_path, framework, description, props_json, events_json, slots_json, tags_json
             FROM components WHERE library_id = ?1 ORDER BY name LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![library_id, limit, offset], |row| {
            Ok(Component {
                id: row.get(0)?,
                library_id: row.get(1)?,
                name: row.get(2)?,
                file_path: row.get(3)?,
                framework: row.get(4)?,
                description: row.get(5)?,
                props: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                events: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                slots: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                tags: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
            })
        })?;
        Ok((rows.filter_map(|r| r.ok()).collect(), total))
    }

    /// Lookup components by exact names (for validate without full-table scan into app memory of unrelated rows).
    pub fn find_components_by_names(
        &self,
        library_id: &str,
        names: &[String],
    ) -> anyhow::Result<Vec<Component>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        let mut out = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT id, library_id, name, file_path, framework, description, props_json, events_json, slots_json, tags_json
             FROM components WHERE library_id = ?1 AND name = ?2",
        )?;
        for name in names {
            let rows = stmt.query_map(params![library_id, name], |row| {
                Ok(Component {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    name: row.get(2)?,
                    file_path: row.get(3)?,
                    framework: row.get(4)?,
                    description: row.get(5)?,
                    props: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    events: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                    slots: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                    tags: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                })
            })?;
            out.extend(rows.filter_map(|r| r.ok()));
        }
        Ok(out)
    }

    pub fn bootstrap_admin_key(&self, key: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM api_keys", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(false);
        }
        drop(conn);
        self.create_api_key("admin", key)?;
        Ok(true)
    }

    pub fn create_api_key(&self, name: &str, key: &str) -> anyhow::Result<ApiKey> {
        let id = Uuid::new_v4().to_string();
        let hash = hash_key(key);
        let prefix = key.chars().take(8).collect::<String>();
        let now = Utc::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, name, key_hash, key_prefix, key_secret, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, hash, prefix, key, now.to_rfc3339()],
        )?;
        Ok(ApiKey {
            id,
            name: name.into(),
            key_prefix: prefix,
            key: Some(key.to_string()),
            created_at: now,
            last_used_at: None,
        })
    }

    pub fn list_api_keys(&self) -> anyhow::Result<Vec<ApiKey>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, key_prefix, key_secret, created_at, last_used_at
             FROM api_keys ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let secret: Option<String> = row.get(3)?;
            Ok(ApiKey {
                id: row.get(0)?,
                name: row.get(1)?,
                key_prefix: row.get(2)?,
                key: secret.filter(|s| !s.is_empty()),
                created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
                last_used_at: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| s.parse().ok()),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Rotate secret for an existing key (keeps id/name). Returns updated record with new secret.
    pub fn regenerate_api_key(&self, id: &str, new_key: &str) -> anyhow::Result<ApiKey> {
        let hash = hash_key(new_key);
        let prefix = new_key.chars().take(8).collect::<String>();
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE api_keys SET key_hash = ?1, key_prefix = ?2, key_secret = ?3 WHERE id = ?4",
            params![hash, prefix, new_key, id],
        )?;
        if updated == 0 {
            anyhow::bail!("API key not found");
        }
        drop(conn);
        self.list_api_keys()?
            .into_iter()
            .find(|k| k.id == id)
            .ok_or_else(|| anyhow::anyhow!("API key not found after regenerate"))
    }

    pub fn delete_api_key(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM api_keys WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn verify_api_key(&self, key: &str) -> anyhow::Result<bool> {
        Ok(self.resolve_api_key(key)?.is_some())
    }

    /// Verify key and return (id, name) when valid.
    pub fn resolve_api_key(&self, key: &str) -> anyhow::Result<Option<(String, String)>> {
        let hash = hash_key(key);
        let conn = self.conn.lock().unwrap();
        let found: Option<(String, String)> = conn
            .query_row(
                "SELECT id, name FROM api_keys WHERE key_hash = ?1",
                [hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, name)) = found {
            conn.execute(
                "UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), id],
            )?;
            return Ok(Some((id, name)));
        }
        Ok(None)
    }

    pub fn create_sync_task(&self, library_id: &str) -> anyhow::Result<SyncTask> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sync_tasks (id, library_id, status, progress, message, started_at)
             VALUES (?1, ?2, 'pending', 0, 'Waiting for job slot...', ?3)",
            params![id, library_id, now.to_rfc3339()],
        )?;
        Ok(SyncTask {
            id,
            library_id: library_id.into(),
            status: "pending".into(),
            progress: 0,
            message: "Waiting for job slot...".into(),
            started_at: now,
            finished_at: None,
        })
    }

    pub fn update_sync_task(
        &self,
        id: &str,
        progress: i32,
        message: &str,
        status: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        if let Some(s) = status {
            let finished = if s == "completed" || s == "failed" {
                Some(Utc::now().to_rfc3339())
            } else {
                None
            };
            conn.execute(
                "UPDATE sync_tasks SET progress = ?1, message = ?2, status = ?3, finished_at = COALESCE(?4, finished_at) WHERE id = ?5",
                params![progress, message, s, finished, id],
            )?;
        } else {
            conn.execute(
                "UPDATE sync_tasks SET progress = ?1, message = ?2 WHERE id = ?3",
                params![progress, message, id],
            )?;
        }
        Ok(())
    }

    pub fn get_sync_task(&self, id: &str) -> anyhow::Result<Option<SyncTask>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, library_id, status, progress, message, started_at, finished_at FROM sync_tasks WHERE id = ?1",
            [id],
            |row| {
                Ok(SyncTask {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    status: row.get(2)?,
                    progress: row.get(3)?,
                    message: row.get(4)?,
                    started_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
                    finished_at: row
                        .get::<_, Option<String>>(6)?
                        .and_then(|s| s.parse().ok()),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_sync_tasks(&self, library_id: Option<&str>) -> anyhow::Result<Vec<SyncTask>> {
        let conn = self.conn.lock().unwrap();
        let (sql, param): (&str, Option<String>) = if let Some(lid) = library_id {
            (
                "SELECT id, library_id, status, progress, message, started_at, finished_at FROM sync_tasks WHERE library_id = ?1 ORDER BY started_at DESC LIMIT 50",
                Some(lid.to_string()),
            )
        } else {
            (
                "SELECT id, library_id, status, progress, message, started_at, finished_at FROM sync_tasks ORDER BY started_at DESC LIMIT 50",
                None,
            )
        };

        let mut stmt = conn.prepare(sql)?;
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(SyncTask {
                id: row.get(0)?,
                library_id: row.get(1)?,
                status: row.get(2)?,
                progress: row.get(3)?,
                message: row.get(4)?,
                started_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
                finished_at: row
                    .get::<_, Option<String>>(6)?
                    .and_then(|s| s.parse().ok()),
            })
        };

        if let Some(p) = param {
            let rows = stmt.query_map([p], map_row)?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        } else {
            let rows = stmt.query_map([], map_row)?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        }
    }

    pub fn stats(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        let libraries: i64 =
            conn.query_row("SELECT COUNT(*) FROM libraries", [], |r| r.get(0))?;
        let components: i64 =
            conn.query_row("SELECT COUNT(*) FROM components", [], |r| r.get(0))?;
        let api_keys: i64 = conn.query_row("SELECT COUNT(*) FROM api_keys", [], |r| r.get(0))?;
        let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        Ok(serde_json::json!({
            "libraries": libraries,
            "components": components,
            "api_keys": api_keys,
            "users": users,
        }))
    }

    pub fn count_users(&self) -> anyhow::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .map_err(Into::into)
    }

    pub fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role: &str,
        display_name: Option<&str>,
    ) -> anyhow::Result<User> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, role, display_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, username, password_hash, role, display_name, now.to_rfc3339()],
        )?;
        Ok(User {
            id,
            username: username.into(),
            role: role.into(),
            display_name: display_name.map(String::from),
            created_at: now,
            last_login_at: None,
        })
    }

    pub fn get_user_by_username(&self, username: &str) -> anyhow::Result<Option<(User, String)>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, username, password_hash, role, display_name, created_at, last_login_at
             FROM users WHERE username = ?1 COLLATE NOCASE",
            [username],
            |row| {
                Ok((
                    User {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        role: row.get(3)?,
                        display_name: row.get(4)?,
                        created_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
                        last_login_at: row
                            .get::<_, Option<String>>(6)?
                            .and_then(|s| s.parse().ok()),
                    },
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_user(&self, id: &str) -> anyhow::Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, username, role, display_name, created_at, last_login_at FROM users WHERE id = ?1",
            [id],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    role: row.get(2)?,
                    display_name: row.get(3)?,
                    created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
                    last_login_at: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| s.parse().ok()),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_users(&self) -> anyhow::Result<Vec<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, role, display_name, created_at, last_login_at FROM users ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                role: row.get(2)?,
                display_name: row.get(3)?,
                created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
                last_login_at: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| s.parse().ok()),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn update_user_password(&self, id: &str, password_hash: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            params![password_hash, id],
        )?;
        Ok(())
    }

    pub fn update_user_username(&self, id: &str, username: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET username = ?1 WHERE id = ?2",
            params![username, id],
        )?;
        Ok(())
    }

    pub fn update_user_role(&self, id: &str, role: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE users SET role = ?1 WHERE id = ?2", params![role, id])?;
        Ok(())
    }

    pub fn delete_user(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM users WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn touch_user_login(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn create_session(&self, user_id: &str, token: &str, expires_at: DateTime<Utc>) -> anyhow::Result<()> {
        let id = Uuid::new_v4().to_string();
        let token_hash = hash_key(token);
        let now = Utc::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, user_id, token_hash, expires_at.to_rfc3339(), now.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn delete_session(&self, token: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            [hash_key(token)],
        )?;
        Ok(())
    }

    /// Delete all sessions for a user, optionally keeping the current token.
    pub fn delete_user_sessions(
        &self,
        user_id: &str,
        except_token: Option<&str>,
    ) -> anyhow::Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n = if let Some(token) = except_token {
            conn.execute(
                "DELETE FROM sessions WHERE user_id = ?1 AND token_hash != ?2",
                params![user_id, hash_key(token)],
            )?
        } else {
            conn.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])?
        };
        Ok(n)
    }

    pub fn session_expires_at(&self, token: &str) -> anyhow::Result<Option<DateTime<Utc>>> {
        let conn = self.conn.lock().unwrap();
        let row: Option<String> = conn
            .query_row(
                "SELECT expires_at FROM sessions WHERE token_hash = ?1",
                [hash_key(token)],
                |r| r.get(0),
            )
            .optional()?;
        Ok(row.and_then(|s| s.parse().ok()))
    }

    pub fn verify_session(&self, token: &str) -> anyhow::Result<Option<User>> {
        Ok(self
            .verify_session_sliding(token, false, 0)?
            .map(|(u, _)| u))
    }

    /// Verify session; when `sliding`, extend expiry to now + extend_hours if later.
    pub fn verify_session_sliding(
        &self,
        token: &str,
        sliding: bool,
        extend_hours: u64,
    ) -> anyhow::Result<Option<(User, DateTime<Utc>)>> {
        let now = Utc::now();
        let conn = self.conn.lock().unwrap();
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT user_id, expires_at FROM sessions WHERE token_hash = ?1 AND expires_at > ?2",
                params![hash_key(token), now.to_rfc3339()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((user_id, expires_raw)) = row else {
            return Ok(None);
        };
        let mut expires: DateTime<Utc> = expires_raw.parse().unwrap_or(now);
        if sliding && extend_hours > 0 {
            let next = now + chrono::Duration::hours(extend_hours as i64);
            if next > expires {
                conn.execute(
                    "UPDATE sessions SET expires_at = ?1 WHERE token_hash = ?2",
                    params![next.to_rfc3339(), hash_key(token)],
                )?;
                expires = next;
            }
        }
        drop(conn);
        Ok(self.get_user(&user_id)?.map(|u| (u, expires)))
    }

    pub fn cleanup_expired_sessions(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM sessions WHERE expires_at <= ?1",
            [Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn bootstrap_admin_user(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<bool> {
        if self.count_users()? > 0 {
            return Ok(false);
        }
        self.create_user(username, password_hash, "admin", Some("Administrator"))?;
        Ok(true)
    }

    pub fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Resolve LLM settings from DB; fill missing fields from env-backed defaults.
    pub fn get_llm_settings(&self, defaults: &crate::config::Config) -> anyhow::Result<LlmSettings> {
        let api_key = self
            .get_setting("llm_api_key")?
            .filter(|s| !s.trim().is_empty())
            .or_else(|| defaults.llm_api_key.clone());
        let base_url = self
            .get_setting("llm_base_url")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| defaults.llm_base_url.clone());
        let model = self
            .get_setting("llm_model")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| defaults.llm_model.clone());
        Ok(LlmSettings {
            api_key,
            base_url,
            model,
        })
    }

    pub fn get_llm_settings_public(
        &self,
        defaults: &crate::config::Config,
    ) -> anyhow::Result<LlmSettingsPublic> {
        let s = self.get_llm_settings(defaults)?;
        Ok(LlmSettingsPublic {
            enabled: s.enabled(),
            api_key_set: s.enabled(),
            api_key_masked: s.mask_key(),
            base_url: s.base_url,
            model: s.model,
        })
    }

    /// Update LLM settings. Empty `api_key` keeps existing key. Empty strings for
    /// base_url/model keep existing (or defaults on next read).
    pub fn update_llm_settings(
        &self,
        api_key: Option<&str>,
        base_url: Option<&str>,
        model: Option<&str>,
        clear_api_key: bool,
    ) -> anyhow::Result<()> {
        if clear_api_key {
            self.set_setting("llm_api_key", "")?;
        } else if let Some(k) = api_key {
            let t = k.trim();
            if !t.is_empty() {
                self.set_setting("llm_api_key", t)?;
            }
        }
        if let Some(u) = base_url {
            let t = u.trim();
            if !t.is_empty() {
                self.set_setting("llm_base_url", t.trim_end_matches('/'))?;
            }
        }
        if let Some(m) = model {
            let t = m.trim();
            if !t.is_empty() {
                self.set_setting("llm_model", t)?;
            }
        }
        Ok(())
    }

    /// Seed settings table from env defaults when keys are missing.
    pub fn bootstrap_llm_settings(&self, defaults: &crate::config::Config) -> anyhow::Result<()> {
        if self.get_setting("llm_base_url")?.is_none() {
            self.set_setting("llm_base_url", &defaults.llm_base_url)?;
        }
        if self.get_setting("llm_model")?.is_none() {
            self.set_setting("llm_model", &defaults.llm_model)?;
        }
        if self.get_setting("llm_api_key")?.is_none() {
            if let Some(k) = &defaults.llm_api_key {
                if !k.trim().is_empty() {
                    self.set_setting("llm_api_key", k)?;
                }
            }
        }
        Ok(())
    }

    fn parse_setting_usize(raw: Option<String>, default: usize) -> usize {
        raw.and_then(|s| s.trim().parse().ok()).unwrap_or(default)
    }

    pub fn get_sync_settings(
        &self,
        defaults: &crate::config::Config,
    ) -> anyhow::Result<crate::db::SyncSettings> {
        Ok(crate::db::SyncSettings {
            max_jobs: Self::parse_setting_usize(
                self.get_setting("sync_max_jobs")?,
                defaults.max_jobs,
            )
            .clamp(1, 64),
            parse_concurrency: Self::parse_setting_usize(
                self.get_setting("sync_parse_concurrency")?,
                defaults.parse_concurrency,
            )
            .min(256),
            ingest_batch_size: Self::parse_setting_usize(
                self.get_setting("sync_ingest_batch_size")?,
                defaults.ingest_batch_size,
            )
            .clamp(20, 5000),
            download_concurrency: Self::parse_setting_usize(
                self.get_setting("sync_download_concurrency")?,
                defaults.download_concurrency,
            )
            .clamp(1, 64),
        })
    }

    pub fn update_sync_settings(
        &self,
        max_jobs: Option<usize>,
        parse_concurrency: Option<usize>,
        ingest_batch_size: Option<usize>,
        download_concurrency: Option<usize>,
    ) -> anyhow::Result<crate::db::SyncSettings> {
        if let Some(v) = max_jobs {
            self.set_setting("sync_max_jobs", &v.clamp(1, 64).to_string())?;
        }
        if let Some(v) = parse_concurrency {
            self.set_setting("sync_parse_concurrency", &v.min(256).to_string())?;
        }
        if let Some(v) = ingest_batch_size {
            self.set_setting("sync_ingest_batch_size", &v.clamp(20, 5000).to_string())?;
        }
        if let Some(v) = download_concurrency {
            self.set_setting("sync_download_concurrency", &v.clamp(1, 64).to_string())?;
        }
        // Caller must pass defaults via get after update — we need config.
        // Return by re-reading with a temporary read of stored values only.
        Ok(crate::db::SyncSettings {
            max_jobs: Self::parse_setting_usize(self.get_setting("sync_max_jobs")?, 3).clamp(1, 64),
            parse_concurrency: Self::parse_setting_usize(
                self.get_setting("sync_parse_concurrency")?,
                0,
            )
            .min(256),
            ingest_batch_size: Self::parse_setting_usize(
                self.get_setting("sync_ingest_batch_size")?,
                250,
            )
            .clamp(20, 5000),
            download_concurrency: Self::parse_setting_usize(
                self.get_setting("sync_download_concurrency")?,
                8,
            )
            .clamp(1, 64),
        })
    }

    pub fn bootstrap_sync_settings(&self, defaults: &crate::config::Config) -> anyhow::Result<()> {
        if self.get_setting("sync_max_jobs")?.is_none() {
            self.set_setting("sync_max_jobs", &defaults.max_jobs.to_string())?;
        }
        if self.get_setting("sync_parse_concurrency")?.is_none() {
            self.set_setting(
                "sync_parse_concurrency",
                &defaults.parse_concurrency.to_string(),
            )?;
        }
        if self.get_setting("sync_ingest_batch_size")?.is_none() {
            self.set_setting(
                "sync_ingest_batch_size",
                &defaults.ingest_batch_size.to_string(),
            )?;
        }
        if self.get_setting("sync_download_concurrency")?.is_none() {
            self.set_setting(
                "sync_download_concurrency",
                &defaults.download_concurrency.to_string(),
            )?;
        }
        Ok(())
    }

    pub fn get_auth_settings(
        &self,
        defaults: &crate::config::Config,
    ) -> anyhow::Result<crate::db::AuthSettings> {
        let sliding = match self.get_setting("auth_session_sliding")? {
            Some(v) => matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            None => defaults.session_sliding,
        };
        Ok(crate::db::AuthSettings {
            session_ttl_hours: Self::parse_setting_u64(
                self.get_setting("auth_session_ttl_hours")?,
                defaults.session_ttl_hours,
            )
            .clamp(1, 24 * 90),
            remember_me_ttl_hours: Self::parse_setting_u64(
                self.get_setting("auth_remember_ttl_hours")?,
                defaults.remember_me_ttl_hours,
            )
            .clamp(1, 24 * 365),
            sliding,
        })
    }

    pub fn update_auth_settings(
        &self,
        session_ttl_hours: Option<u64>,
        remember_me_ttl_hours: Option<u64>,
        sliding: Option<bool>,
        defaults: &crate::config::Config,
    ) -> anyhow::Result<crate::db::AuthSettings> {
        if let Some(v) = session_ttl_hours {
            self.set_setting(
                "auth_session_ttl_hours",
                &v.clamp(1, 24 * 90).to_string(),
            )?;
        }
        if let Some(v) = remember_me_ttl_hours {
            self.set_setting(
                "auth_remember_ttl_hours",
                &v.clamp(1, 24 * 365).to_string(),
            )?;
        }
        if let Some(v) = sliding {
            self.set_setting("auth_session_sliding", if v { "true" } else { "false" })?;
        }
        self.get_auth_settings(defaults)
    }

    pub fn bootstrap_auth_settings(&self, defaults: &crate::config::Config) -> anyhow::Result<()> {
        if self.get_setting("auth_session_ttl_hours")?.is_none() {
            self.set_setting(
                "auth_session_ttl_hours",
                &defaults.session_ttl_hours.to_string(),
            )?;
        }
        if self.get_setting("auth_remember_ttl_hours")?.is_none() {
            self.set_setting(
                "auth_remember_ttl_hours",
                &defaults.remember_me_ttl_hours.to_string(),
            )?;
        }
        if self.get_setting("auth_session_sliding")?.is_none() {
            self.set_setting(
                "auth_session_sliding",
                if defaults.session_sliding {
                    "true"
                } else {
                    "false"
                },
            )?;
        }
        Ok(())
    }

    fn parse_setting_u64(raw: Option<String>, default: u64) -> u64 {
        raw.and_then(|s| s.trim().parse().ok()).unwrap_or(default)
    }

    pub fn storage_table_stats(&self) -> anyhow::Result<crate::db::StorageTableStats> {
        let conn = self.conn.lock().unwrap();
        let components_rows: u64 = conn
            .query_row("SELECT COUNT(*) FROM components", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as u64;
        let libraries_rows: u64 = conn
            .query_row("SELECT COUNT(*) FROM libraries", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as u64;
        let sync_tasks_rows: u64 = conn
            .query_row("SELECT COUNT(*) FROM sync_tasks", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as u64;
        let app_logs_rows: u64 = conn
            .query_row("SELECT COUNT(*) FROM app_logs", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as u64;
        let sessions_rows: u64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as u64;
        let components_source_bytes: u64 = conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(source_content)), 0) FROM components",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) as u64;
        Ok(crate::db::StorageTableStats {
            components_rows,
            libraries_rows,
            sync_tasks_rows,
            app_logs_rows,
            sessions_rows,
            components_source_bytes,
        })
    }

    pub fn sqlite_page_bytes(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok((page_count as u64).saturating_mul(page_size as u64))
    }

    pub fn prune_sync_tasks(&self, older_than_days: u64, dry_run: bool) -> anyhow::Result<usize> {
        let cutoff = (Utc::now() - chrono::Duration::days(older_than_days as i64)).to_rfc3339();
        let conn = self.conn.lock().unwrap();
        if dry_run {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sync_tasks
                 WHERE status IN ('completed','failed','success','error','cancelled')
                   AND COALESCE(finished_at, started_at) < ?1",
                [&cutoff],
                |r| r.get(0),
            )?;
            return Ok(n as usize);
        }
        let n = conn.execute(
            "DELETE FROM sync_tasks
             WHERE status IN ('completed','failed','success','error','cancelled')
               AND COALESCE(finished_at, started_at) < ?1",
            [&cutoff],
        )?;
        Ok(n)
    }

    pub fn prune_app_logs(&self, older_than_days: u64, dry_run: bool) -> anyhow::Result<usize> {
        let cutoff = (Utc::now() - chrono::Duration::days(older_than_days as i64)).to_rfc3339();
        let conn = self.conn.lock().unwrap();
        if dry_run {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM app_logs WHERE created_at < ?1",
                [&cutoff],
                |r| r.get(0),
            )?;
            return Ok(n as usize);
        }
        let n = conn.execute("DELETE FROM app_logs WHERE created_at < ?1", [&cutoff])?;
        Ok(n)
    }

    pub fn prune_expired_sessions_count(&self, dry_run: bool) -> anyhow::Result<usize> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        if dry_run {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE expires_at <= ?1",
                [&now],
                |r| r.get(0),
            )?;
            return Ok(n as usize);
        }
        let n = conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [&now])?;
        Ok(n)
    }

    pub fn wal_checkpoint_truncate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub fn vacuum(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("VACUUM;")?;
        Ok(())
    }

    pub fn get_cleanup_schedule(&self) -> anyhow::Result<CleanupSchedule> {
        let defaults = crate::storage::default_cleanup_schedule();
        let enabled = match self.get_setting("cleanup_enabled")? {
            Some(v) => matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            None => defaults.enabled,
        };
        Ok(CleanupSchedule {
            enabled,
            interval_hours: Self::parse_setting_u64(
                self.get_setting("cleanup_interval_hours")?,
                defaults.interval_hours,
            )
            .clamp(1, 24 * 30),
            orphan_repos: Self::parse_setting_bool(
                self.get_setting("cleanup_orphan_repos")?,
                defaults.orphan_repos,
            ),
            sync_tasks_older_than_days: Self::parse_setting_u64(
                self.get_setting("cleanup_sync_tasks_days")?,
                defaults.sync_tasks_older_than_days,
            )
            .min(3650),
            app_logs_older_than_days: Self::parse_setting_u64(
                self.get_setting("cleanup_app_logs_days")?,
                defaults.app_logs_older_than_days,
            )
            .min(3650),
            expired_sessions: Self::parse_setting_bool(
                self.get_setting("cleanup_expired_sessions")?,
                defaults.expired_sessions,
            ),
            wal_checkpoint: Self::parse_setting_bool(
                self.get_setting("cleanup_wal_checkpoint")?,
                defaults.wal_checkpoint,
            ),
            vacuum: Self::parse_setting_bool(self.get_setting("cleanup_vacuum")?, defaults.vacuum),
            last_run_at: self.get_setting("cleanup_last_run_at")?,
            last_result: self.get_setting("cleanup_last_result")?,
        })
    }

    pub fn update_cleanup_schedule(
        &self,
        patch: &CleanupSchedule,
    ) -> anyhow::Result<CleanupSchedule> {
        self.set_setting(
            "cleanup_enabled",
            if patch.enabled { "true" } else { "false" },
        )?;
        self.set_setting(
            "cleanup_interval_hours",
            &patch.interval_hours.clamp(1, 24 * 30).to_string(),
        )?;
        self.set_setting(
            "cleanup_orphan_repos",
            if patch.orphan_repos { "true" } else { "false" },
        )?;
        self.set_setting(
            "cleanup_sync_tasks_days",
            &patch.sync_tasks_older_than_days.min(3650).to_string(),
        )?;
        self.set_setting(
            "cleanup_app_logs_days",
            &patch.app_logs_older_than_days.min(3650).to_string(),
        )?;
        self.set_setting(
            "cleanup_expired_sessions",
            if patch.expired_sessions {
                "true"
            } else {
                "false"
            },
        )?;
        self.set_setting(
            "cleanup_wal_checkpoint",
            if patch.wal_checkpoint {
                "true"
            } else {
                "false"
            },
        )?;
        self.set_setting(
            "cleanup_vacuum",
            if patch.vacuum { "true" } else { "false" },
        )?;
        self.get_cleanup_schedule()
    }

    pub fn record_cleanup_run(&self, at: &str, summary: &str) -> anyhow::Result<()> {
        self.set_setting("cleanup_last_run_at", at)?;
        self.set_setting("cleanup_last_result", summary)?;
        Ok(())
    }

    pub fn bootstrap_cleanup_settings(&self) -> anyhow::Result<()> {
        let d = crate::storage::default_cleanup_schedule();
        if self.get_setting("cleanup_enabled")?.is_none() {
            self.set_setting("cleanup_enabled", "false")?;
        }
        if self.get_setting("cleanup_interval_hours")?.is_none() {
            self.set_setting("cleanup_interval_hours", &d.interval_hours.to_string())?;
        }
        if self.get_setting("cleanup_orphan_repos")?.is_none() {
            self.set_setting("cleanup_orphan_repos", "true")?;
        }
        if self.get_setting("cleanup_sync_tasks_days")?.is_none() {
            self.set_setting(
                "cleanup_sync_tasks_days",
                &d.sync_tasks_older_than_days.to_string(),
            )?;
        }
        if self.get_setting("cleanup_app_logs_days")?.is_none() {
            self.set_setting(
                "cleanup_app_logs_days",
                &d.app_logs_older_than_days.to_string(),
            )?;
        }
        if self.get_setting("cleanup_expired_sessions")?.is_none() {
            self.set_setting("cleanup_expired_sessions", "true")?;
        }
        if self.get_setting("cleanup_wal_checkpoint")?.is_none() {
            self.set_setting("cleanup_wal_checkpoint", "true")?;
        }
        if self.get_setting("cleanup_vacuum")?.is_none() {
            self.set_setting("cleanup_vacuum", "false")?;
        }
        Ok(())
    }

    fn parse_setting_bool(raw: Option<String>, default: bool) -> bool {
        match raw {
            Some(v) => matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            None => default,
        }
    }
}

pub fn hash_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn generate_api_key() -> String {
    format!("cmcp_{}", Uuid::new_v4().simple())
}
