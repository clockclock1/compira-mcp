//! In-memory ring buffer of recent MCP / playground tool calls for the live UI.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

const RING_CAP: usize = 300;
const ARGS_MAX: usize = 240;

#[derive(Debug, Clone, Serialize)]
pub struct McpCaller {
    pub source: String,
    pub api_key_id: Option<String>,
    pub api_key_name: Option<String>,
    pub user_id: Option<String>,
    pub username: Option<String>,
}

impl McpCaller {
    pub fn mcp(api_key_id: Option<String>, api_key_name: Option<String>) -> Self {
        Self {
            source: "mcp".into(),
            api_key_id,
            api_key_name,
            user_id: None,
            username: None,
        }
    }

    pub fn playground(user_id: Option<String>, username: Option<String>) -> Self {
        Self {
            source: "playground".into(),
            api_key_id: None,
            api_key_name: None,
            user_id,
            username,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct McpCallEvent {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub tool: String,
    pub args_summary: String,
    pub duration_ms: Option<u64>,
    pub ok: Option<bool>,
    pub error: Option<String>,
    pub source: String,
    pub api_key_id: Option<String>,
    pub api_key_name: Option<String>,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub status: String, // running | ok | error
}

#[derive(Clone)]
pub struct McpActivity {
    inner: std::sync::Arc<Mutex<VecDeque<McpCallEvent>>>,
    tx: broadcast::Sender<McpCallEvent>,
}

impl McpActivity {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            inner: std::sync::Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAP))),
            tx,
        }
    }

    pub fn begin(&self, tool: &str, args: &Value, caller: &McpCaller) -> String {
        let id = Uuid::new_v4().to_string();
        let event = McpCallEvent {
            id: id.clone(),
            started_at: Utc::now(),
            finished_at: None,
            tool: tool.to_string(),
            args_summary: summarize_args(args),
            duration_ms: None,
            ok: None,
            error: None,
            source: caller.source.clone(),
            api_key_id: caller.api_key_id.clone(),
            api_key_name: caller.api_key_name.clone(),
            user_id: caller.user_id.clone(),
            username: caller.username.clone(),
            status: "running".into(),
        };
        self.push(event);
        id
    }

    pub fn finish(&self, id: &str, ok: bool, error: Option<String>, started: Instant) {
        let duration_ms = started.elapsed().as_millis() as u64;
        let mut guard = self.inner.lock().unwrap();
        if let Some(ev) = guard.iter_mut().find(|e| e.id == id) {
            ev.finished_at = Some(Utc::now());
            ev.duration_ms = Some(duration_ms);
            ev.ok = Some(ok);
            ev.error = error;
            ev.status = if ok { "ok".into() } else { "error".into() };
            let _ = self.tx.send(ev.clone());
        }
    }

    pub fn list(&self, limit: usize) -> Vec<McpCallEvent> {
        let guard = self.inner.lock().unwrap();
        guard.iter().rev().take(limit.clamp(1, RING_CAP)).cloned().collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }

    pub fn subscribe(&self) -> broadcast::Receiver<McpCallEvent> {
        self.tx.subscribe()
    }

    fn push(&self, event: McpCallEvent) {
        let mut guard = self.inner.lock().unwrap();
        if guard.len() >= RING_CAP {
            guard.pop_front();
        }
        let _ = self.tx.send(event.clone());
        guard.push_back(event);
    }
}

impl Default for McpActivity {
    fn default() -> Self {
        Self::new()
    }
}

/// Summarize args for the live feed; redact large `code` bodies.
pub fn summarize_args(args: &Value) -> String {
    let mut v = args.clone();
    if let Some(obj) = v.as_object_mut() {
        if let Some(code) = obj.get("code").and_then(|c| c.as_str()) {
            let preview: String = code.chars().take(80).collect();
            obj.insert(
                "code".into(),
                Value::String(format!("{preview}… ({} chars)", code.len())),
            );
        }
    }
    let s = serde_json::to_string(&v).unwrap_or_else(|_| "{}".into());
    if s.len() <= ARGS_MAX {
        s
    } else {
        format!("{}…", &s[..ARGS_MAX])
    }
}

tokio::task_local! {
    /// Set by /mcp auth middleware so tool handlers can attribute the caller.
    pub static CURRENT_MCP_CALLER: McpCaller;
}

pub fn current_caller_or(fallback: McpCaller) -> McpCaller {
    CURRENT_MCP_CALLER
        .try_with(|c| c.clone())
        .unwrap_or(fallback)
}
