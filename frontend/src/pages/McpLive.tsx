import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api/client";
import { useAuth } from "../auth/AuthContext";

export type McpCallEvent = {
  id: string;
  started_at: string;
  finished_at: string | null;
  tool: string;
  args_summary: string;
  duration_ms: number | null;
  ok: boolean | null;
  error: string | null;
  source: string;
  api_key_id: string | null;
  api_key_name: string | null;
  user_id: string | null;
  username: string | null;
  status: string;
};

function formatTime(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function statusChip(ev: McpCallEvent) {
  if (ev.status === "running") return { text: "进行中", cls: "chip-status" };
  if (ev.ok) return { text: "成功", cls: "chip-status chip-green" };
  return { text: "失败", cls: "chip-status chip-red" };
}

/**
 * Live MCP tool-call monitor (polls every second).
 */
export default function McpLive() {
  const { isAdmin } = useAuth();
  const [events, setEvents] = useState<McpCallEvent[]>([]);
  const [error, setError] = useState("");
  const [paused, setPaused] = useState(false);
  const [filter, setFilter] = useState("");
  const [sourceFilter, setSourceFilter] = useState<"all" | "mcp" | "playground">("all");
  const [selected, setSelected] = useState<McpCallEvent | null>(null);

  const load = useCallback(() => {
    if (paused) return;
    api.mcp
      .activity(100)
      .then((list) => {
        setEvents(list);
        setError("");
      })
      .catch((e) => setError(e.message));
  }, [paused]);

  useEffect(() => {
    load();
    const t = window.setInterval(load, 1000);
    return () => window.clearInterval(t);
  }, [load]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return events.filter((e) => {
      if (sourceFilter !== "all" && e.source !== sourceFilter) return false;
      if (!q) return true;
      return (
        e.tool.toLowerCase().includes(q) ||
        e.args_summary.toLowerCase().includes(q) ||
        (e.api_key_name || "").toLowerCase().includes(q) ||
        (e.username || "").toLowerCase().includes(q) ||
        (e.error || "").toLowerCase().includes(q)
      );
    });
  }, [events, filter, sourceFilter]);

  const running = events.filter((e) => e.status === "running").length;
  const okCount = events.filter((e) => e.ok === true).length;
  const errCount = events.filter((e) => e.ok === false).length;

  const clear = async () => {
    if (!confirm("清空当前实时调用记录？")) return;
    try {
      await api.mcp.clearActivity();
      setEvents([]);
      setSelected(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "清空失败");
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>MCP 实时调用</h1>
          <div className="page-sub">
            监听 Cursor / Agent 经 <code>/mcp</code> 与控制台调试调用 · 每秒刷新
          </div>
        </div>
        <div className="header-actions">
          <div className={`chip-status ${running ? "chip-green" : ""}`}>
            <span className="dot" />
            {running ? `${running} 进行中` : "监听中"}
          </div>
          <Link to="/mcp" className="btn btn-ghost">
            打开调试台
          </Link>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => setPaused((p) => !p)}
          >
            {paused ? "继续" : "暂停"}
          </button>
          {isAdmin && (
            <button type="button" className="btn btn-ghost" onClick={clear}>
              清空
            </button>
          )}
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      <div className="mcp-live-stats">
        <div className="maint-stat">
          <div className="maint-stat-label">缓冲内调用</div>
          <div className="maint-stat-value">{events.length}</div>
        </div>
        <div className="maint-stat">
          <div className="maint-stat-label">成功</div>
          <div className="maint-stat-value" style={{ color: "var(--green)" }}>
            {okCount}
          </div>
        </div>
        <div className="maint-stat">
          <div className="maint-stat-label">失败</div>
          <div className="maint-stat-value" style={{ color: "var(--red, #fb7185)" }}>
            {errCount}
          </div>
        </div>
        <div className="maint-stat">
          <div className="maint-stat-label">进行中</div>
          <div className="maint-stat-value">{running}</div>
        </div>
      </div>

      <div className="mcp-live-toolbar">
        <input
          className="modal-input"
          style={{ maxWidth: 280 }}
          placeholder="过滤工具 / 参数 / Key…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <div className="mcp-live-tabs">
          {(
            [
              ["all", "全部"],
              ["mcp", "远程 MCP"],
              ["playground", "调试台"],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              className={`tab-item${sourceFilter === key ? " active" : ""}`}
              onClick={() => setSourceFilter(key)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="mcp-live-layout">
        <div className="card mcp-live-list">
          <div className="card-header">
            <div className="card-title">调用流</div>
            <span className="chip-count">{filtered.length}</span>
          </div>
          <div className="divider" />
          <div className="mcp-live-scroll">
            {filtered.length === 0 ? (
              <div className="empty" style={{ padding: 32 }}>
                暂无调用。用 Cursor 连上 /mcp，或到「MCP 调试」发一次请求。
              </div>
            ) : (
              filtered.map((ev) => {
                const chip = statusChip(ev);
                const active = selected?.id === ev.id;
                return (
                  <button
                    key={ev.id}
                    type="button"
                    className={`mcp-live-row${active ? " active" : ""}`}
                    onClick={() => setSelected(ev)}
                  >
                    <div className="mcp-live-row-top">
                      <span className="mcp-live-tool">{ev.tool}</span>
                      <span className={chip.cls}>
                        <span className="dot" />
                        {chip.text}
                      </span>
                    </div>
                    <div className="mcp-live-row-meta">
                      <span>{formatTime(ev.started_at)}</span>
                      <span>{ev.source === "mcp" ? "远程" : "调试"}</span>
                      <span>
                        {ev.api_key_name || ev.username || "—"}
                      </span>
                      <span>
                        {ev.duration_ms != null ? `${ev.duration_ms}ms` : "…"}
                      </span>
                    </div>
                    <div className="mcp-live-args">{ev.args_summary}</div>
                  </button>
                );
              })
            )}
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <div className="card-title">详情</div>
          </div>
          <div className="divider" />
          <div className="card-body">
            {!selected ? (
              <div className="empty">选择左侧一条调用查看详情</div>
            ) : (
              <div className="mcp-live-detail">
                <div className="mcp-live-kv">
                  <span>工具</span>
                  <strong>{selected.tool}</strong>
                </div>
                <div className="mcp-live-kv">
                  <span>来源</span>
                  <strong>
                    {selected.source === "mcp" ? "远程 MCP (/mcp)" : "控制台调试"}
                  </strong>
                </div>
                <div className="mcp-live-kv">
                  <span>调用方</span>
                  <strong>
                    {selected.api_key_name
                      ? `API Key · ${selected.api_key_name}`
                      : selected.username
                        ? `用户 · ${selected.username}`
                        : "未知"}
                  </strong>
                </div>
                <div className="mcp-live-kv">
                  <span>开始</span>
                  <strong>{new Date(selected.started_at).toLocaleString()}</strong>
                </div>
                <div className="mcp-live-kv">
                  <span>耗时</span>
                  <strong>
                    {selected.duration_ms != null ? `${selected.duration_ms} ms` : "进行中…"}
                  </strong>
                </div>
                <div className="mcp-live-kv">
                  <span>状态</span>
                  <strong>{statusChip(selected).text}</strong>
                </div>
                {selected.error && (
                  <div className="error" style={{ marginTop: 12 }}>
                    {selected.error}
                  </div>
                )}
                <div className="resp-label" style={{ marginTop: 16 }}>
                  参数摘要
                </div>
                <pre className="code-block" style={{ whiteSpace: "pre-wrap" }}>
                  {selected.args_summary}
                </pre>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
