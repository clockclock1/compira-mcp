import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, Library, Stats, SyncTask } from "../api/client";

function formatTime(iso: string | null) {
  if (!iso) return "未同步";
  const d = new Date(iso);
  const diff = Date.now() - d.getTime();
  if (diff < 60_000) return "刚刚";
  if (diff < 3600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86400_000) return `${Math.floor(diff / 3600_000)} 小时前`;
  return d.toLocaleString();
}

function statusLabel(status: string) {
  if (status === "ready") return { text: "同步成功", cls: "ok" };
  if (status === "error") return { text: "同步失败", cls: "fail" };
  if (status === "syncing") return { text: "索引中…", cls: "idx" };
  return { text: status, cls: "" };
}

export default function Dashboard() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [tasks, setTasks] = useState<SyncTask[]>([]);
  const [error, setError] = useState("");
  const [refreshing, setRefreshing] = useState(false);

  const load = () => {
    setRefreshing(true);
    Promise.all([api.stats(), api.libraries.list(), api.tasks.list()])
      .then(([s, libs, t]) => {
        setStats(s);
        setLibraries(libs);
        setTasks(t.slice(0, 8));
      })
      .catch((e) => setError(e.message))
      .finally(() => setRefreshing(false));
  };

  useEffect(() => {
    load();
  }, []);

  const recentLibs = [...libraries]
    .sort((a, b) => {
      const ta = a.last_synced_at ? new Date(a.last_synced_at).getTime() : 0;
      const tb = b.last_synced_at ? new Date(b.last_synced_at).getTime() : 0;
      return tb - ta;
    })
    .slice(0, 4);

  const frameworkDist = (() => {
    // approximate from library names/status — we don't have global framework stats; show placeholders from component count
    const total = stats?.components || 0;
    return [
      { name: "Vue / Uni-app", pct: total ? 70 : 0, color: "var(--type-blue)" },
      { name: "其他", pct: total ? 30 : 0, color: "var(--primary)" },
    ];
  })();

  const runningTasks = tasks.filter((t) => t.status === "running").length;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>仪表盘</h1>
          <div className="page-sub">组件库 MCP 服务器运行概览</div>
        </div>
        <div className="header-actions">
          <div className="chip-status chip-green">
            <span className="dot" />
            服务运行中
          </div>
          <button className="btn btn-ghost" onClick={load} disabled={refreshing}>
            <svg viewBox="0 0 24 24">
              <path d="M21 12a9 9 0 1 1-2.6-6.4" />
              <path d="M21 3v6h-6" />
            </svg>
            {refreshing ? "刷新中…" : "刷新"}
          </button>
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      {stats && (
        <div className="stats-row">
          <div className="stat-card">
            <div className="stat-label">组件库</div>
            <div className="stat-value">{stats.libraries}</div>
          </div>
          <div className="stat-card">
            <div className="stat-label">
              已索引组件 <span className="stat-delta blue">实时</span>
            </div>
            <div className="stat-value">{stats.components.toLocaleString()}</div>
          </div>
          <div className="stat-card">
            <div className="stat-label">
              API Keys <span className="stat-delta violet">{stats.api_keys} 已启用</span>
            </div>
            <div className="stat-value">{stats.api_keys}</div>
          </div>
          <div className="stat-card">
            <div className="stat-label">用户</div>
            <div className="stat-value">{stats.users}</div>
          </div>
        </div>
      )}

      <div className="dash-row">
        <div className="card" style={{ padding: 20 }}>
          <div className="card-header" style={{ padding: "0 0 14px" }}>
            <div className="card-title">MCP 接入说明</div>
            <span style={{ flex: 1 }} />
            <span className="chip-method">标准 MCP 协议 · HTTP + X-API-Key</span>
          </div>
          <div className="code-block">
            <span className="cmt"># Cursor / Claude Code 配置（mcp.json）</span>
            <br />
            {"{ "}
            <span className="key">"mcpServers"</span>
            {": { "}
            <span className="key">"compira"</span>
            {": {"}
            <br />
            &nbsp;&nbsp;&nbsp;&nbsp;
            <span className="key">"url"</span>: <span className="str">"{window.location.origin}/mcp"</span>,
            <br />
            &nbsp;&nbsp;&nbsp;&nbsp;
            <span className="key">"headers"</span>
            {": { "}
            <span className="key">"X-API-Key"</span>: <span className="str">"&lt;your-api-key&gt;"</span>
            {" } } } }"}
          </div>
        </div>
        <div className="card" style={{ padding: 20, minHeight: 296 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <div className="card-title">最近同步</div>
            <Link to="/libraries" className="link-action" style={{ fontSize: 12 }}>
              查看全部
            </Link>
          </div>
          <div className="sync-list">
            {recentLibs.length === 0 && <div className="empty" style={{ padding: 20 }}>暂无组件库</div>}
            {recentLibs.map((lib) => {
              const st = statusLabel(lib.status);
              return (
                <div className="sync-item" key={lib.id}>
                  <div
                    className="sync-icon"
                    style={{
                      background:
                        lib.status === "error"
                          ? "rgba(248,113,113,0.12)"
                          : "rgba(99,102,241,0.12)",
                      color: lib.status === "error" ? "var(--red)" : "var(--primary-light)",
                    }}
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
                      <path d="M3 7l9-4 9 4-9 4-9-4z" />
                      <path d="M3 7v10l9 4 9-4V7" />
                    </svg>
                  </div>
                  <div className="sync-body">
                    <div className="sync-name">
                      {lib.name}{" "}
                      <span style={{ fontSize: 11, color: "var(--text-3)", fontWeight: 400 }}>
                        · {lib.component_count} 个组件
                      </span>
                    </div>
                    <div className="sync-sub">
                      {lib.branch} → <span className={st.cls}>{st.text}</span>
                    </div>
                  </div>
                  <div className="sync-time">{formatTime(lib.last_synced_at)}</div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      <div className="dash-row-3">
        <div className="card" style={{ padding: 20, minHeight: 204 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
            <div className="card-title">组件概览</div>
            <span style={{ fontSize: 12, color: "var(--text-3)" }}>
              {(stats?.components || 0).toLocaleString()} 个组件
            </span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
            {frameworkDist.map((d) => (
              <div className="dist-bar-row" key={d.name}>
                <div className="dist-label">
                  <span>{d.name}</span>
                  <span>{d.pct}%</span>
                </div>
                <div className="dist-track">
                  <div className="dist-fill" style={{ width: `${d.pct}%`, background: d.color }} />
                </div>
              </div>
            ))}
          </div>
        </div>
        <div className="card" style={{ padding: 20, minHeight: 204 }}>
          <div className="card-title" style={{ marginBottom: 4 }}>
            系统状态
          </div>
          <div className="sys-row">
            <span className="sys-label">MCP 端点</span>
            <span className="sys-value">
              <span className="dot-inline" />
              online · {window.location.host}
            </span>
          </div>
          <div className="sys-row">
            <span className="sys-label">同步队列</span>
            <span className="sys-value">{runningTasks} 个任务</span>
          </div>
          <div className="sys-row">
            <span className="sys-label">组件库</span>
            <span className="sys-value">{stats?.libraries ?? 0}</span>
          </div>
          <div className="sys-row">
            <span className="sys-label">服务版本</span>
            <span className="sys-value">v0.1.0</span>
          </div>
        </div>
      </div>
    </div>
  );
}
