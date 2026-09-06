import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { api, ApiKey, Library, Stats, SyncTask } from "../api/client";
import AlertBanner from "../components/AlertBanner";
import CopyButton from "../components/CopyButton";
import EmptyState from "../components/EmptyState";
import { StatSkeleton } from "../components/Skeleton";
import {
  CountUp,
  GlassCard,
  PageMotion,
  Stagger,
  StaggerItem,
} from "../components/motion";

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

function sourceLabel(t?: string) {
  if (t === "upload") return "上传";
  if (t === "fetch") return "AI 拉取";
  return "Git";
}

export default function Dashboard() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [tasks, setTasks] = useState<SyncTask[]>([]);
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [error, setError] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [syncingAll, setSyncingAll] = useState(false);
  const [syncMsg, setSyncMsg] = useState("");

  const load = () => {
    setRefreshing(true);
    Promise.all([api.stats(), api.libraries.list(), api.tasks.list(), api.apiKeys.list()])
      .then(([s, libs, t, keys]) => {
        setStats(s);
        setLibraries(libs);
        setTasks(t.slice(0, 8));
        setApiKeys(keys);
        setError("");
      })
      .catch((e) => setError(e.message))
      .finally(() => {
        setRefreshing(false);
        setLoaded(true);
      });
  };

  /**
   * Enqueue sync for every library; actual parallelism is capped by settings.
   */
  const handleSyncAll = async () => {
    if (libraries.length === 0) return;
    setSyncingAll(true);
    setSyncMsg("");
    setError("");
    let ok = 0;
    const errors: string[] = [];
    for (const lib of libraries) {
      try {
        await api.libraries.sync(lib.id);
        ok += 1;
      } catch (err) {
        errors.push(`${lib.name}: ${err instanceof Error ? err.message : "失败"}`);
      }
    }
    setSyncingAll(false);
    if (errors.length) {
      setError(`已入队 ${ok} 个；失败 ${errors.length}：${errors.slice(0, 3).join("；")}`);
    } else {
      setSyncMsg(`已一键入队 ${ok} 个组件库同步（并行数见「系统设置」）`);
    }
    load();
  };

  useEffect(() => {
    load();
  }, []);

  const recentLibs = useMemo(
    () =>
      [...libraries]
        .sort((a, b) => {
          const ta = a.last_synced_at ? new Date(a.last_synced_at).getTime() : 0;
          const tb = b.last_synced_at ? new Date(b.last_synced_at).getTime() : 0;
          return tb - ta;
        })
        .slice(0, 5),
    [libraries],
  );

  const sourceDist = useMemo(() => {
    const buckets: Record<string, { name: string; count: number; color: string }> = {
      git: { name: "Git 同步", count: 0, color: "var(--accent-2)" },
      fetch: { name: "AI 拉取", count: 0, color: "var(--violet)" },
      upload: { name: "本地上传", count: 0, color: "var(--accent)" },
    };
    for (const lib of libraries) {
      const key = lib.source_type || "git";
      const b = buckets[key] || buckets.git;
      b.count += lib.component_count || 0;
    }
    const total = Object.values(buckets).reduce((s, b) => s + b.count, 0) || 1;
    return Object.values(buckets)
      .filter((b) => b.count > 0)
      .map((b) => ({ ...b, pct: Math.round((b.count * 100) / total) }));
  }, [libraries]);

  const runningTasks = tasks.filter(
    (t) => t.status === "running" || t.status === "pending",
  ).length;
  const failedLibs = libraries.filter((l) => l.status === "error").length;

  /** Earliest created key that still has a recoverable secret. */
  const firstApiKey = useMemo(() => {
    const withSecret = apiKeys
      .filter((k) => !!k.key?.trim())
      .sort(
        (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      );
    return withSecret[0] || null;
  }, [apiKeys]);

  const mcpApiKeyDisplay = firstApiKey?.key?.trim() || "<your-api-key>";

  const mcpSnippet = `{
  "mcpServers": {
    "compira": {
      "url": "${window.location.origin}/mcp",
      "headers": {
        "X-API-Key": "${mcpApiKeyDisplay}"
      }
    }
  }
}`;

  return (
    <PageMotion>
      <div className="page-header">
        <div>
          <h1>仪表盘</h1>
          <div className="page-sub">实时运维概览 · 玻璃态面板 · 组件索引状态</div>
        </div>
        <div className="header-actions">
          <div className="chip-status chip-green">
            <span className="dot" />
            服务运行中
          </div>
          {failedLibs > 0 && (
            <Link to="/libraries" className="chip-status chip-red">
              <span className="dot dot-red" />
              {failedLibs} 个库失败
            </Link>
          )}
          <button
            className="btn btn-primary"
            onClick={handleSyncAll}
            disabled={syncingAll || libraries.length === 0}
            title="一键同步全部组件库（并行数可在系统设置配置）"
          >
            <svg viewBox="0 0 24 24">
              <path d="M21 12a9 9 0 1 1-2.6-6.4" />
              <path d="M21 3v6h-6" />
            </svg>
            {syncingAll ? "入队中…" : "一键同步"}
          </button>
          <button className="btn btn-ghost" onClick={load} disabled={refreshing}>
            <svg viewBox="0 0 24 24">
              <path d="M21 12a9 9 0 1 1-2.6-6.4" />
              <path d="M21 3v6h-6" />
            </svg>
            {refreshing ? "刷新中…" : "刷新"}
          </button>
        </div>
      </div>

      {error && (
        <AlertBanner title="加载失败" message={error} onClose={() => setError("")} />
      )}
      {syncMsg && !error && (
        <p style={{ color: "var(--green)", fontSize: 12.5, marginBottom: 12 }}>{syncMsg}</p>
      )}

      {!loaded && <StatSkeleton count={4} />}

      {loaded && stats && (
        <Stagger className="stats-row">
          <StaggerItem className="stat-card">
            <div className="stat-label">
              组件库
              <span className="stat-icon" aria-hidden>
                <svg viewBox="0 0 24 24" width="16" height="16">
                  <path d="M3 7l9-4 9 4-9 4-9-4z" />
                  <path d="M3 7v10l9 4 9-4V7" />
                </svg>
              </span>
            </div>
            <div className="stat-value">
              <CountUp value={stats.libraries} />
            </div>
            <div className="stat-unit">已注册仓库</div>
          </StaggerItem>
          <StaggerItem className="stat-card">
            <div className="stat-label">
              已索引组件
              <span className="stat-delta blue">实时</span>
            </div>
            <div className="stat-value">
              <CountUp value={stats.components} />
            </div>
            <div className="stat-unit">可供 MCP 检索</div>
          </StaggerItem>
          <StaggerItem className="stat-card">
            <div className="stat-label">
              API Keys
              <span className="stat-delta violet">{stats.api_keys} 启用</span>
            </div>
            <div className="stat-value">
              <CountUp value={stats.api_keys} />
            </div>
            <div className="stat-unit">
              <Link to="/api-keys" className="link-action" style={{ fontSize: 12 }}>
                管理密钥
              </Link>
            </div>
          </StaggerItem>
          <StaggerItem className="stat-card">
            <div className="stat-label">同步队列</div>
            <div className="stat-value">
              <CountUp value={runningTasks} />
            </div>
            <div className="stat-unit">{runningTasks ? "任务进行中" : "空闲"}</div>
          </StaggerItem>
        </Stagger>
      )}

      <div className="dash-row">
        <GlassCard style={{ padding: 20 }}>
          <div className="card-header" style={{ padding: "0 0 14px" }}>
            <div className="card-title">MCP 接入</div>
            <span style={{ flex: 1 }} />
            <span className="chip-method">HTTP · X-API-Key</span>
            <CopyButton text={mcpSnippet} label="复制配置" />
          </div>
          {!firstApiKey && (
            <div style={{ fontSize: 12, color: "var(--text-3)", marginBottom: 10 }}>
              暂无可用密钥，请到{" "}
              <Link to="/api-keys" className="link-action">
                API Keys
              </Link>{" "}
              创建后自动填入
            </div>
          )}
          <div className="code-block">
            <span className="cmt"># Cursor mcp.json</span>
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
            <span className="key">"X-API-Key"</span>: <span className="str">"{mcpApiKeyDisplay}"</span>
            {" } } } }"}
          </div>
        </GlassCard>
        <GlassCard style={{ padding: 20, minHeight: 296 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <div className="card-title">最近同步</div>
            <Link to="/libraries" className="link-action" style={{ fontSize: 12 }}>
              查看全部
            </Link>
          </div>
          <div className="sync-list">
            {loaded && recentLibs.length === 0 && (
              <EmptyState
                title="还没有组件库"
                description="通过 Git、上传或 AI 拉取导入组件，即可被 Cursor 检索"
                actionLabel="添加组件库"
                actionTo="/libraries"
              />
            )}
            {recentLibs.map((lib) => {
              const st = statusLabel(lib.status);
              return (
                <Link to={`/libraries/${lib.id}`} className="sync-item sync-item-link" key={lib.id}>
                  <div
                    className="sync-icon"
                    style={{
                      background:
                        lib.status === "error"
                          ? "rgba(251,113,133,0.12)"
                          : "rgba(45,212,191,0.12)",
                      color: lib.status === "error" ? "var(--red)" : "var(--accent)",
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
                      <span className="chip-tag" style={{ marginLeft: 6 }}>
                        {sourceLabel(lib.source_type)}
                      </span>
                    </div>
                    <div className="sync-sub">
                      {lib.component_count.toLocaleString()} 组件 ·{" "}
                      <span className={st.cls}>{st.text}</span>
                    </div>
                  </div>
                  <div className="sync-time">{formatTime(lib.last_synced_at)}</div>
                </Link>
              );
            })}
          </div>
        </GlassCard>
      </div>

      <div className="dash-row-3">
        <GlassCard style={{ padding: 20, minHeight: 204 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
            <div className="card-title">组件来源分布</div>
            <span style={{ fontSize: 12, color: "var(--text-3)" }}>
              {(stats?.components || 0).toLocaleString()} 个组件
            </span>
          </div>
          {sourceDist.length === 0 ? (
            <EmptyState title="暂无索引数据" description="导入组件库后将显示来源占比" />
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              {sourceDist.map((d) => (
                <div className="dist-bar-row" key={d.name}>
                  <div className="dist-label">
                    <span>{d.name}</span>
                    <span>
                      {d.pct}% · {d.count.toLocaleString()}
                    </span>
                  </div>
                  <div className="dist-track">
                    <div className="dist-fill" style={{ width: `${d.pct}%`, background: d.color }} />
                  </div>
                </div>
              ))}
            </div>
          )}
        </GlassCard>
        <GlassCard style={{ padding: 20, minHeight: 204 }} spotlight={false}>
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
            <span className="sys-label">失败库</span>
            <span className="sys-value" style={{ color: failedLibs ? "var(--red)" : undefined }}>
              {failedLibs}
            </span>
          </div>
          <div className="sys-row">
            <span className="sys-label">用户</span>
            <span className="sys-value">{stats?.users ?? "—"}</span>
          </div>
        </GlassCard>
      </div>
    </PageMotion>
  );
}
