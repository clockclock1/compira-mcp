import { FormEvent, useCallback, useEffect, useState } from "react";
import { api } from "../api/client";

type StorageStats = Awaited<ReturnType<typeof api.admin.storageStats>>;
type MemoryReport = Awaited<ReturnType<typeof api.admin.memory>>;
type CleanupSchedule = Awaited<ReturnType<typeof api.admin.cleanupSchedule>>;
type CleanupResult = Awaited<ReturnType<typeof api.admin.cleanup>>;

function fmtBytes(n: number) {
  if (!n || n < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/**
 * Admin panel: disk usage, memory breakdown, one-click + scheduled cleanup.
 */
export default function MaintenancePanel({
  onError,
  onOk,
}: {
  onError: (msg: string) => void;
  onOk: (msg: string) => void;
}) {
  const [stats, setStats] = useState<StorageStats | null>(null);
  const [memory, setMemory] = useState<MemoryReport | null>(null);
  const [schedule, setSchedule] = useState<CleanupSchedule | null>(null);
  const [loading, setLoading] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [savingSched, setSavingSched] = useState(false);
  const [lastResult, setLastResult] = useState<CleanupResult | null>(null);
  const [doVacuum, setDoVacuum] = useState(false);

  const [enabled, setEnabled] = useState(false);
  const [intervalHours, setIntervalHours] = useState("24");
  const [orphanRepos, setOrphanRepos] = useState(true);
  const [taskDays, setTaskDays] = useState("30");
  const [logDays, setLogDays] = useState("14");
  const [expSessions, setExpSessions] = useState(true);
  const [walCp, setWalCp] = useState(true);
  const [schedVacuum, setSchedVacuum] = useState(false);

  const refresh = useCallback(() => {
    setLoading(true);
    Promise.all([
      api.admin.storageStats(),
      api.admin.memory(),
      api.admin.cleanupSchedule(),
    ])
      .then(([s, m, sch]) => {
        setStats(s);
        setMemory(m);
        setSchedule(sch);
        setEnabled(sch.enabled);
        setIntervalHours(String(sch.interval_hours));
        setOrphanRepos(sch.orphan_repos);
        setTaskDays(String(sch.sync_tasks_older_than_days));
        setLogDays(String(sch.app_logs_older_than_days));
        setExpSessions(sch.expired_sessions);
        setWalCp(sch.wal_checkpoint);
        setSchedVacuum(sch.vacuum);
      })
      .catch((e) => onError(e.message))
      .finally(() => setLoading(false));
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const runCleanup = async (dryRun: boolean) => {
    setCleaning(true);
    onError("");
    try {
      const r = await api.admin.cleanup({
        dry_run: dryRun,
        orphan_repos: true,
        sync_tasks_older_than_days: Number(taskDays) || 30,
        app_logs_older_than_days: Number(logDays) || 14,
        expired_sessions: true,
        wal_checkpoint: true,
        vacuum: !dryRun && doVacuum,
      });
      setLastResult(r);
      if (r.errors.length) {
        onError(r.errors.slice(0, 3).join("；"));
      }
      onOk(
        dryRun
          ? `预览：可删孤儿 ${r.orphan_repos_removed}、任务 ${r.sync_tasks_deleted}、日志 ${r.app_logs_deleted}`
          : `清理完成：孤儿 ${r.orphan_repos_removed}、任务 ${r.sync_tasks_deleted}、日志 ${r.app_logs_deleted}`,
      );
      if (!dryRun) refresh();
    } catch (e) {
      onError(e instanceof Error ? e.message : "清理失败");
    } finally {
      setCleaning(false);
    }
  };

  const saveSchedule = async (e: FormEvent) => {
    e.preventDefault();
    setSavingSched(true);
    onError("");
    try {
      const updated = await api.admin.updateCleanupSchedule({
        enabled,
        interval_hours: Math.max(1, Number(intervalHours) || 24),
        orphan_repos: orphanRepos,
        sync_tasks_older_than_days: Math.max(0, Number(taskDays) || 0),
        app_logs_older_than_days: Math.max(0, Number(logDays) || 0),
        expired_sessions: expSessions,
        wal_checkpoint: walCp,
        vacuum: schedVacuum,
        last_run_at: schedule?.last_run_at,
        last_result: schedule?.last_result,
      });
      setSchedule(updated);
      onOk(
        updated.enabled
          ? `定时清理已开启（每 ${updated.interval_hours} 小时）`
          : "定时清理已关闭",
      );
    } catch (err) {
      onError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSavingSched(false);
    }
  };

  return (
    <section className="card settings-card settings-card-wide">
      <div className="card-header">
        <div>
          <div className="card-title">存储与维护</div>
          <div className="settings-card-desc">
            查看磁盘 / 内存占用 · 一键清理无用数据 · 定时维护
          </div>
        </div>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={refresh}
          disabled={loading}
        >
          {loading ? "刷新中…" : "刷新占用"}
        </button>
      </div>
      <div className="divider" />
      <div className="card-body">
        {stats && (
          <div className="maint-stats">
            <div className="maint-stat">
              <div className="maint-stat-label">总占用</div>
              <div className="maint-stat-value">{fmtBytes(stats.total_bytes)}</div>
              <div className="hint-text">{stats.data_dir}</div>
            </div>
            <div className="maint-stat">
              <div className="maint-stat-label">数据库</div>
              <div className="maint-stat-value">
                {fmtBytes(stats.database.file_bytes + stats.database.wal_bytes)}
              </div>
              <div className="hint-text">
                库文件 {fmtBytes(stats.database.file_bytes)} · WAL{" "}
                {fmtBytes(stats.database.wal_bytes)} · 源码列{" "}
                {fmtBytes(stats.database.components_source_bytes)}
              </div>
            </div>
            <div className="maint-stat">
              <div className="maint-stat-label">Repos 目录</div>
              <div className="maint-stat-value">{fmtBytes(stats.repos.total_bytes)}</div>
              <div className="hint-text">
                {stats.repos.library_dirs} 个目录 · 孤儿 {stats.repos.orphan_dirs} · 可回收约{" "}
                {fmtBytes(stats.reclaimable_bytes_estimate)}
              </div>
            </div>
            <div className="maint-stat">
              <div className="maint-stat-label">表行数</div>
              <div className="maint-stat-value" style={{ fontSize: 18 }}>
                {stats.database.components_rows} 组件
              </div>
              <div className="hint-text">
                任务 {stats.database.sync_tasks_rows} · 日志 {stats.database.app_logs_rows} ·
                会话 {stats.database.sessions_rows}
              </div>
            </div>
          </div>
        )}

        {memory && (
          <div className="maint-memory" style={{ marginTop: 18 }}>
            <div className="card-title" style={{ fontSize: 14, marginBottom: 10 }}>
              内存占用分析
              <span className="chip-file" style={{ marginLeft: 10 }}>
                PID {memory.process.pid} · WS {fmtBytes(memory.process.working_set_bytes)} ·
                虚拟 {fmtBytes(memory.process.virtual_bytes)}
              </span>
            </div>
            <div className="hint-text" style={{ marginBottom: 10 }}>
              当前同步任务 {memory.task_manager.active_jobs} / {memory.task_manager.max_jobs}
            </div>
            <ul className="maint-breakdown">
              {memory.breakdown.map((b) => (
                <li key={b.key}>
                  <div className="maint-bd-top">
                    <strong>{b.label}</strong>
                    <span>{b.bytes != null ? fmtBytes(b.bytes) : "—"}</span>
                  </div>
                  <div className="hint-text">{b.detail}</div>
                </li>
              ))}
            </ul>
            <ul className="maint-notes">
              {memory.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </div>
        )}

        {stats && stats.repos.entries.length > 0 && (
          <div style={{ marginTop: 16, overflowX: "auto" }}>
            <div className="card-title" style={{ fontSize: 14, marginBottom: 8 }}>
              各库磁盘占用
            </div>
            <table className="table" style={{ fontSize: 12.5 }}>
              <thead>
                <tr>
                  <th>名称</th>
                  <th>类型</th>
                  <th>大小</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                {stats.repos.entries.slice(0, 20).map((e) => (
                  <tr key={e.id}>
                    <td>{e.name || e.id.slice(0, 8)}</td>
                    <td>{e.source_type || "—"}</td>
                    <td>{fmtBytes(e.bytes)}</td>
                    <td>{e.orphan ? <span className="chip-status chip-red">孤儿</span> : "在用"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <div className="divider" style={{ margin: "18px 0" }} />

        <div className="card-title" style={{ fontSize: 14, marginBottom: 8 }}>
          一键清理
        </div>
        <p className="hint-text" style={{ marginBottom: 12 }}>
          安全清理：孤儿 repos 目录、过期会话、旧同步任务 / 日志、WAL checkpoint。不会删除仍在用的组件库。
        </p>
        <label className="settings-check" style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          <input
            type="checkbox"
            checked={doVacuum}
            onChange={(e) => setDoVacuum(e.target.checked)}
          />
          <span>
            同时 VACUUM 数据库
            <span className="hint-text" style={{ display: "block" }}>
              可回收更多空间，但会短暂锁库，数据量大时较慢
            </span>
          </span>
        </label>
        <div className="settings-actions" style={{ marginTop: 0 }}>
          <button
            type="button"
            className="btn btn-ghost"
            disabled={cleaning}
            onClick={() => runCleanup(true)}
          >
            预览清理
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={cleaning}
            onClick={() => {
              if (
                confirm(
                  doVacuum
                    ? "确认一键清理并 VACUUM？可能短暂锁库。"
                    : "确认一键清理无用文件与历史记录？",
                )
              ) {
                void runCleanup(false);
              }
            }}
          >
            {cleaning ? "清理中…" : "一键清理"}
          </button>
        </div>
        {lastResult && (
          <pre className="maint-result">
            {[
              ...lastResult.messages,
              ...lastResult.errors.map((e) => `ERR: ${e}`),
              `估计释放文件约 ${fmtBytes(lastResult.bytes_freed_estimate)}`,
            ].join("\n")}
          </pre>
        )}

        <div className="divider" style={{ margin: "18px 0" }} />

        <div className="card-title" style={{ fontSize: 14, marginBottom: 8 }}>
          定时清理
        </div>
        <form onSubmit={saveSchedule}>
          <label className="settings-check" style={{ display: "flex", gap: 8, marginBottom: 12 }}>
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
            />
            <span>启用定时清理</span>
          </label>
          <div className="settings-fields-2">
            <div className="settings-field">
              <label htmlFor="clean-interval">间隔（小时）</label>
              <input
                id="clean-interval"
                className="modal-input"
                type="number"
                min={1}
                max={720}
                value={intervalHours}
                onChange={(e) => setIntervalHours(e.target.value)}
              />
            </div>
            <div className="settings-field">
              <label htmlFor="clean-tasks">任务保留（天）</label>
              <input
                id="clean-tasks"
                className="modal-input"
                type="number"
                min={0}
                max={3650}
                value={taskDays}
                onChange={(e) => setTaskDays(e.target.value)}
              />
            </div>
            <div className="settings-field">
              <label htmlFor="clean-logs">日志保留（天）</label>
              <input
                id="clean-logs"
                className="modal-input"
                type="number"
                min={0}
                max={3650}
                value={logDays}
                onChange={(e) => setLogDays(e.target.value)}
              />
            </div>
          </div>
          <div className="maint-checks" style={{ marginTop: 12 }}>
            <label>
              <input
                type="checkbox"
                checked={orphanRepos}
                onChange={(e) => setOrphanRepos(e.target.checked)}
              />{" "}
              孤儿 repos
            </label>
            <label>
              <input
                type="checkbox"
                checked={expSessions}
                onChange={(e) => setExpSessions(e.target.checked)}
              />{" "}
              过期会话
            </label>
            <label>
              <input type="checkbox" checked={walCp} onChange={(e) => setWalCp(e.target.checked)} />{" "}
              WAL checkpoint
            </label>
            <label>
              <input
                type="checkbox"
                checked={schedVacuum}
                onChange={(e) => setSchedVacuum(e.target.checked)}
              />{" "}
              定时 VACUUM（不推荐常开）
            </label>
          </div>
          {schedule?.last_run_at && (
            <p className="hint-text" style={{ marginTop: 10 }}>
              上次运行：{new Date(schedule.last_run_at).toLocaleString()}
              {schedule.last_result ? ` · ${schedule.last_result}` : ""}
            </p>
          )}
          <div className="settings-actions">
            <button type="submit" className="btn btn-primary" disabled={savingSched}>
              {savingSched ? "保存中…" : "保存定时策略"}
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}
