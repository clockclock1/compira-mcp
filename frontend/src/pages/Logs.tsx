import { useEffect, useMemo, useState } from "react";
import { api, LogEntry } from "../api/client";

function normalizeLevel(level: string) {
  return level.toLowerCase();
}

export default function Logs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [error, setError] = useState("");
  const [filter, setFilter] = useState<"all" | "info" | "warn" | "error">("all");

  const load = () => {
    api
      .logs(200)
      .then(setLogs)
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    load();
    const interval = setInterval(load, 5000);
    return () => clearInterval(interval);
  }, []);

  const filtered = useMemo(() => {
    if (filter === "all") return logs;
    return logs.filter((l) => normalizeLevel(l.level) === filter);
  }, [logs, filter]);

  const timeClass = (level: string) => {
    const n = normalizeLevel(level);
    if (n === "warn" || n === "warning") return "warn";
    if (n === "error") return "error";
    return "";
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>日志</h1>
          <div className="page-sub">服务端运行日志（MCP Server · 索引器 · 鉴权）</div>
        </div>
        <button className="btn btn-ghost btn-ghost-sm" onClick={load}>
          <svg viewBox="0 0 24 24">
            <path d="M21 12a9 9 0 1 1-2.6-6.4" />
            <path d="M21 3v6h-6" />
          </svg>
          刷新
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      <div className="card log-card">
        <div className="log-toolbar">
          <div className="filter-chips">
            {(
              [
                ["all", "全部"],
                ["info", "INFO"],
                ["warn", "WARN"],
                ["error", "ERROR"],
              ] as const
            ).map(([key, label]) => (
              <span
                key={key}
                className={`chip-filter${filter === key ? " active" : ""}`}
                onClick={() => setFilter(key)}
              >
                {label}
              </span>
            ))}
          </div>
          <span className="log-count">
            共 <span>{filtered.length}</span> 条
          </span>
        </div>
        <div className="divider" />
        <div className="log-list">
          {filtered.length === 0 ? (
            <div className="empty" style={{ padding: 24 }}>
              暂无日志
            </div>
          ) : (
            filtered.map((log) => (
              <div className="log-line" key={log.id} data-level={log.level}>
                <span className={`log-time ${timeClass(log.level)}`}>
                  [{new Date(log.created_at).toLocaleTimeString()}]
                </span>
                <span className="log-msg">
                  {normalizeLevel(log.level).toUpperCase()}  {log.message}
                  {log.context ? `  (${log.context})` : ""}
                </span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
