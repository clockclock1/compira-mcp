//! Disk / DB maintenance: storage stats, safe cleanup, process memory report.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};
use walkdir::WalkDir;

use crate::config::Config;
use crate::db::{CleanupSchedule, Database};
use crate::git;
use crate::tasks::TaskManager;

#[derive(Debug, Clone, Serialize)]
pub struct DirSizeEntry {
    pub id: String,
    pub name: Option<String>,
    pub bytes: u64,
    pub source_type: Option<String>,
    pub orphan: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageStats {
    pub data_dir: String,
    pub repos_dir: String,
    pub total_bytes: u64,
    pub database: DatabaseDiskStats,
    pub repos: ReposDiskStats,
    pub reclaimable_bytes_estimate: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseDiskStats {
    pub path: String,
    pub file_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub page_bytes: u64,
    pub components_source_bytes: u64,
    pub components_rows: u64,
    pub libraries_rows: u64,
    pub sync_tasks_rows: u64,
    pub app_logs_rows: u64,
    pub sessions_rows: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReposDiskStats {
    pub total_bytes: u64,
    pub library_dirs: usize,
    pub orphan_dirs: usize,
    pub entries: Vec<DirSizeEntry>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CleanupResult {
    pub dry_run: bool,
    pub orphan_repos_removed: usize,
    pub orphan_repos_bytes: u64,
    pub sync_tasks_deleted: usize,
    pub app_logs_deleted: usize,
    pub sessions_deleted: usize,
    pub wal_checkpoint: bool,
    pub vacuum: bool,
    pub bytes_freed_estimate: u64,
    pub messages: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryBreakdownItem {
    pub key: String,
    pub label: String,
    pub bytes: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryReport {
    pub process: ProcessMemory,
    pub task_manager: TaskMemoryInfo,
    pub sqlite: SqliteMemoryInfo,
    pub breakdown: Vec<MemoryBreakdownItem>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessMemory {
    pub pid: u32,
    pub working_set_bytes: u64,
    pub virtual_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskMemoryInfo {
    pub max_jobs: usize,
    pub active_jobs: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SqliteMemoryInfo {
    /// Soft cache target from PRAGMA cache_size (KiB when negative).
    pub cache_size_kib: i64,
    pub mmap_size: u64,
    pub temp_store: String,
}

#[derive(Debug, Clone)]
pub struct CleanupOptions {
    pub dry_run: bool,
    pub orphan_repos: bool,
    pub sync_tasks_older_than_days: Option<u64>,
    pub app_logs_older_than_days: Option<u64>,
    pub expired_sessions: bool,
    pub wal_checkpoint: bool,
    pub vacuum: bool,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            orphan_repos: true,
            sync_tasks_older_than_days: Some(30),
            app_logs_older_than_days: Some(14),
            expired_sessions: true,
            wal_checkpoint: true,
            vacuum: false,
        }
    }
}

impl From<&CleanupSchedule> for CleanupOptions {
    fn from(s: &CleanupSchedule) -> Self {
        Self {
            dry_run: false,
            orphan_repos: s.orphan_repos,
            sync_tasks_older_than_days: if s.sync_tasks_older_than_days > 0 {
                Some(s.sync_tasks_older_than_days)
            } else {
                None
            },
            app_logs_older_than_days: if s.app_logs_older_than_days > 0 {
                Some(s.app_logs_older_than_days)
            } else {
                None
            },
            expired_sessions: s.expired_sessions,
            wal_checkpoint: s.wal_checkpoint,
            vacuum: s.vacuum,
        }
    }
}

pub fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

pub fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn collect_storage_stats(db: &Database, config: &Config) -> anyhow::Result<StorageStats> {
    let db_path = PathBuf::from(&config.database_url);
    let wal_path = PathBuf::from(format!("{}-wal", config.database_url));
    let shm_path = PathBuf::from(format!("{}-shm", config.database_url));

    let table = db.storage_table_stats()?;
    let page_bytes = db.sqlite_page_bytes().unwrap_or(0);

    let libraries = db.list_libraries()?;
    let known: HashSet<String> = libraries.iter().map(|l| l.id.clone()).collect();
    let name_by_id: std::collections::HashMap<String, (String, String)> = libraries
        .iter()
        .map(|l| (l.id.clone(), (l.name.clone(), l.source_type.clone())))
        .collect();

    let mut entries = Vec::new();
    let mut repos_total = 0u64;
    let mut orphan_dirs = 0usize;
    let mut orphan_bytes = 0u64;

    if config.repos_dir.is_dir() {
        for entry in std::fs::read_dir(&config.repos_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            let bytes = dir_size(&entry.path());
            repos_total += bytes;
            let orphan = !known.contains(&id);
            if orphan {
                orphan_dirs += 1;
                orphan_bytes += bytes;
            }
            let (name, source_type) = name_by_id
                .get(&id)
                .cloned()
                .map(|(n, s)| (Some(n), Some(s)))
                .unwrap_or((None, None));
            entries.push(DirSizeEntry {
                id,
                name,
                bytes,
                source_type,
                orphan,
            });
        }
    }

    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    let db_file = file_size(&db_path);
    let wal_bytes = file_size(&wal_path);
    let shm_bytes = file_size(&shm_path);
    let database = DatabaseDiskStats {
        path: config.database_url.clone(),
        file_bytes: db_file,
        wal_bytes,
        shm_bytes,
        page_bytes,
        components_source_bytes: table.components_source_bytes,
        components_rows: table.components_rows,
        libraries_rows: table.libraries_rows,
        sync_tasks_rows: table.sync_tasks_rows,
        app_logs_rows: table.app_logs_rows,
        sessions_rows: table.sessions_rows,
    };

    let total_bytes = db_file + wal_bytes + shm_bytes + repos_total;
    // Rough reclaimable: orphans + WAL (after checkpoint) + old rows (unknown size, omit bytes).
    let reclaimable = orphan_bytes + wal_bytes;

    Ok(StorageStats {
        data_dir: config.data_dir.display().to_string(),
        repos_dir: config.repos_dir.display().to_string(),
        total_bytes,
        database,
        repos: ReposDiskStats {
            total_bytes: repos_total,
            library_dirs: entries.len(),
            orphan_dirs,
            entries,
        },
        reclaimable_bytes_estimate: reclaimable,
    })
}

pub fn run_cleanup(
    db: &Database,
    config: &Config,
    opts: CleanupOptions,
) -> anyhow::Result<CleanupResult> {
    let mut result = CleanupResult {
        dry_run: opts.dry_run,
        ..Default::default()
    };

    if opts.orphan_repos {
        let libraries = db.list_libraries()?;
        let known: HashSet<String> = libraries.into_iter().map(|l| l.id).collect();
        if config.repos_dir.is_dir() {
            for entry in std::fs::read_dir(&config.repos_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let id = entry.file_name().to_string_lossy().to_string();
                if known.contains(&id) {
                    continue;
                }
                let bytes = dir_size(&entry.path());
                result.orphan_repos_bytes += bytes;
                result.orphan_repos_removed += 1;
                if opts.dry_run {
                    result
                        .messages
                        .push(format!("[dry-run] would remove orphan repo {id} ({bytes} bytes)"));
                } else if let Err(e) = git::remove_repo(&entry.path()) {
                    result.errors.push(format!("orphan {id}: {e}"));
                } else {
                    result.messages.push(format!("removed orphan repo {id}"));
                }
            }
        }
    }

    if let Some(days) = opts.sync_tasks_older_than_days {
        match db.prune_sync_tasks(days, opts.dry_run) {
            Ok(n) => {
                result.sync_tasks_deleted = n;
                if n > 0 {
                    result.messages.push(format!(
                        "{} {n} finished sync_tasks older than {days}d",
                        if opts.dry_run { "would delete" } else { "deleted" }
                    ));
                }
            }
            Err(e) => result.errors.push(format!("prune sync_tasks: {e}")),
        }
    }

    if let Some(days) = opts.app_logs_older_than_days {
        match db.prune_app_logs(days, opts.dry_run) {
            Ok(n) => {
                result.app_logs_deleted = n;
                if n > 0 {
                    result.messages.push(format!(
                        "{} {n} app_logs older than {days}d",
                        if opts.dry_run { "would delete" } else { "deleted" }
                    ));
                }
            }
            Err(e) => result.errors.push(format!("prune app_logs: {e}")),
        }
    }

    if opts.expired_sessions {
        match db.prune_expired_sessions_count(opts.dry_run) {
            Ok(n) => {
                result.sessions_deleted = n;
                if n > 0 {
                    result.messages.push(format!(
                        "{} {n} expired sessions",
                        if opts.dry_run { "would delete" } else { "deleted" }
                    ));
                }
            }
            Err(e) => result.errors.push(format!("prune sessions: {e}")),
        }
    }

    if opts.wal_checkpoint && !opts.dry_run {
        match db.wal_checkpoint_truncate() {
            Ok(()) => {
                result.wal_checkpoint = true;
                result.messages.push("WAL checkpoint (TRUNCATE) done".into());
            }
            Err(e) => result.errors.push(format!("wal_checkpoint: {e}")),
        }
    } else if opts.wal_checkpoint && opts.dry_run {
        result.messages.push("[dry-run] would run WAL checkpoint".into());
    }

    if opts.vacuum && !opts.dry_run {
        match db.vacuum() {
            Ok(()) => {
                result.vacuum = true;
                result.messages.push("VACUUM completed".into());
            }
            Err(e) => result.errors.push(format!("vacuum: {e}")),
        }
    } else if opts.vacuum && opts.dry_run {
        result
            .messages
            .push("[dry-run] would VACUUM database (locks DB, may take time)".into());
    }

    result.bytes_freed_estimate = result.orphan_repos_bytes;
    Ok(result)
}

pub fn collect_memory_report(db: &Database, tasks: &TaskManager) -> MemoryReport {
    let pid = std::process::id();
    let (ws, virt) = process_memory_bytes(pid);

    let sqlite = SqliteMemoryInfo {
        cache_size_kib: 65536, // matches PRAGMA cache_size = -65536
        mmap_size: 256 * 1024 * 1024,
        temp_store: "MEMORY".into(),
    };

    let active = tasks.active_jobs();
    let max_jobs = tasks.max_jobs();

    let mut breakdown = vec![
        MemoryBreakdownItem {
            key: "working_set".into(),
            label: "进程工作集 (Working Set)".into(),
            bytes: Some(ws),
            detail: "操作系统看到的进程常驻内存，含代码、堆、映射页".into(),
        },
        MemoryBreakdownItem {
            key: "sqlite_cache".into(),
            label: "SQLite 页缓存目标".into(),
            bytes: Some((sqlite.cache_size_kib as u64) * 1024),
            detail: "PRAGMA cache_size≈64MiB，加速查询；随负载涨落".into(),
        },
        MemoryBreakdownItem {
            key: "sqlite_mmap".into(),
            label: "SQLite mmap 上限".into(),
            bytes: Some(sqlite.mmap_size),
            detail: "PRAGMA mmap_size=256MiB；多为虚拟映射，不一定全部进工作集".into(),
        },
        MemoryBreakdownItem {
            key: "task_queue".into(),
            label: "同步任务并发".into(),
            bytes: None,
            detail: format!("当前运行 {active} / 上限 {max_jobs}；每个任务可能带 Rayon 解析线程与源码批次"),
        },
    ];

    if let Ok(t) = db.storage_table_stats() {
        breakdown.push(MemoryBreakdownItem {
            key: "source_in_db".into(),
            label: "组件源码（磁盘 DB，非常驻堆）".into(),
            bytes: Some(t.components_source_bytes),
            detail: format!(
                "{} 个组件的 source_content 存在 SQLite；查询时按页载入缓存",
                t.components_rows
            ),
        });
    }

    MemoryReport {
        process: ProcessMemory {
            pid,
            working_set_bytes: ws,
            virtual_bytes: virt,
        },
        task_manager: TaskMemoryInfo {
            max_jobs,
            active_jobs: active,
        },
        sqlite,
        breakdown,
        notes: vec![
            "无独立组件 LRU 缓存；MCP 读库不另建内存索引".into(),
            "Rayon 解析线程池按任务临时创建，任务结束后释放".into(),
            "AI 拉取时的下载信号量与批次缓冲仅在任务期间占用".into(),
            "降低「同步并行 / 解析并发 / 入库批次」可减少峰值内存".into(),
        ],
    }
}

fn process_memory_bytes(pid: u32) -> (u64, u64) {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    if let Some(p) = sys.process(Pid::from_u32(pid)) {
        (p.memory(), p.virtual_memory())
    } else {
        (0, 0)
    }
}

/// Human-friendly summary for schedule last_result.
pub fn format_cleanup_summary(r: &CleanupResult) -> String {
    format!(
        "orphans={}, tasks={}, logs={}, sessions={}, wal={}, vacuum={}, errors={}",
        r.orphan_repos_removed,
        r.sync_tasks_deleted,
        r.app_logs_deleted,
        r.sessions_deleted,
        r.wal_checkpoint,
        r.vacuum,
        r.errors.len()
    )
}

pub fn default_cleanup_schedule() -> CleanupSchedule {
    CleanupSchedule {
        enabled: false,
        interval_hours: 24,
        orphan_repos: true,
        sync_tasks_older_than_days: 30,
        app_logs_older_than_days: 14,
        expired_sessions: true,
        wal_checkpoint: true,
        vacuum: false,
        last_run_at: None,
        last_result: None,
    }
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}
