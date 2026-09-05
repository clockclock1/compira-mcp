import { Link, useParams } from "react-router-dom";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { api, Component, Library } from "../api/client";

export default function LibraryDetail() {
  const { id } = useParams<{ id: string }>();
  const [library, setLibrary] = useState<Library | null>(null);
  const [components, setComponents] = useState<Component[]>([]);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [fwFilter, setFwFilter] = useState("全部");
  const [busy, setBusy] = useState(false);
  const [aiEnabled, setAiEnabled] = useState(false);
  const [showUpload, setShowUpload] = useState(false);
  const [showFetch, setShowFetch] = useState(false);
  const [files, setFiles] = useState<FileList | null>(null);
  const [useAi, setUseAi] = useState(true);
  const [fetchPrompt, setFetchPrompt] = useState("");

  const load = useCallback(() => {
    if (!id) return;
    Promise.all([api.libraries.get(id), api.libraries.components(id)])
      .then(([lib, comps]) => {
        setLibrary(lib);
        setComponents(comps);
      })
      .catch((e) => setError(e.message));
  }, [id]);

  useEffect(() => {
    load();
    api.ai.status().then((s) => setAiEnabled(s.enabled)).catch(() => {});
  }, [load]);

  useEffect(() => {
    if (!id) return;
    const interval = setInterval(() => {
      api.tasks.list(id).then((tasks) => {
        const running = tasks.some((t) => t.status === "running" || t.status === "pending");
        if (!running) load();
      }).catch(() => {});
    }, 2500);
    return () => clearInterval(interval);
  }, [id, load]);

  const frameworks = useMemo(() => {
    const set = new Set(components.map((c) => c.framework).filter(Boolean));
    return ["全部", ...Array.from(set)];
  }, [components]);

  const filtered = components.filter((c) => {
    const kwOk =
      !search ||
      c.name.toLowerCase().includes(search.toLowerCase()) ||
      c.file_path.toLowerCase().includes(search.toLowerCase());
    const fwOk = fwFilter === "全部" || c.framework === fwFilter;
    return kwOk && fwOk;
  });

  const sourceType = library?.source_type || "git";

  const handleSync = async () => {
    if (!id) return;
    setBusy(true);
    setError("");
    try {
      await api.libraries.sync(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : "同步失败");
    } finally {
      setBusy(false);
    }
  };

  const handleUpload = async (e: FormEvent) => {
    e.preventDefault();
    if (!id || !files?.length) return;
    setBusy(true);
    setError("");
    try {
      await api.libraries.upload(id, Array.from(files), useAi);
      setShowUpload(false);
      setFiles(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "上传失败");
    } finally {
      setBusy(false);
    }
  };

  const handleFetch = async (e: FormEvent) => {
    e.preventDefault();
    if (!id) return;
    if (!fetchPrompt.trim()) {
      setError("请填写自然语言需求描述");
      return;
    }
    if (!aiEnabled) {
      setError("AI 拉取需先在「系统设置」配置 LLM API Key");
      return;
    }
    setBusy(true);
    setError("");
    try {
      await api.libraries.fetch(id, { prompt: fetchPrompt.trim() });
      setShowFetch(false);
      setFetchPrompt("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "拉取失败");
    } finally {
      setBusy(false);
    }
  };

  if (!library && !error) {
    return <div className="loading">加载中...</div>;
  }

  const statusOk = library?.status === "ready";

  return (
    <div className="page">
      <div className="crumb">
        <Link to="/libraries">组件库</Link>
        <span className="sep">/</span>
        <span className="cur">{library?.name}</span>
      </div>

      <div className="page-header" style={{ marginBottom: 20 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
          <h1>{library?.name}</h1>
          <span className="chip-version">{library?.branch}</span>
          <span className="chip-tag">
            {sourceType === "upload" ? "上传" : sourceType === "fetch" ? "AI 拉取" : "Git"}
          </span>
          <span className="chip-status chip-green" style={{ padding: "4px 10px", fontSize: 11.5 }}>
            <span className="dot" style={{ width: 6, height: 6 }} />
            {statusOk ? "已同步" : library?.status || "-"}
          </span>
        </div>
        <div className="header-actions">
          <button className="btn btn-ghost" onClick={() => setShowUpload(true)}>
            上传组件
          </button>
          <button className="btn btn-ghost" onClick={() => setShowFetch(true)}>
            AI 拉取
          </button>
          {sourceType === "git" && (
            <button className="btn btn-ghost" onClick={handleSync} disabled={busy}>
              {busy ? "同步中…" : "立即同步"}
            </button>
          )}
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      {!aiEnabled && (
        <div className="tip-banner">
          <svg viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="9" />
            <path d="M12 8v4" />
            <path d="M12 16h.01" />
          </svg>
          <span>
            未配置 LLM：上传仍会用本地解析入库；AI 拉取请到「系统设置」填写 API Key / Base URL / Model。
          </span>
        </div>
      )}

      <div className="info-grid">
        <div className="info-card">
          <div className="info-label">组件数</div>
          <div className="info-value">{library?.component_count ?? 0}</div>
        </div>
        <div className="info-card">
          <div className="info-label">上游</div>
          <div className="info-value mono" style={{ fontSize: 12 }}>
            {library?.repo_url.startsWith("upload://") || library?.repo_url.startsWith("fetch://")
              ? "本地入库"
              : library?.repo_url.replace(/^https?:\/\//, "").replace(/\.git$/, "")}
          </div>
        </div>
        <div className="info-card">
          <div className="info-label">分支</div>
          <div className="info-value mono">{library?.branch}</div>
        </div>
        <div className="info-card">
          <div className="info-label">最近同步</div>
          <div className="info-value" style={{ fontSize: 13 }}>
            <span className="dot-inline" />
            {library?.last_synced_at
              ? new Date(library.last_synced_at).toLocaleString()
              : "未同步"}
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">已索引组件</div>
          <span className="chip-count">{components.length}</span>
          <span style={{ flex: 1 }} />
          <div className="search-box" style={{ width: 220, height: 34 }}>
            <svg viewBox="0 0 24 24">
              <circle cx="11" cy="11" r="7" />
              <path d="M21 21l-4.3-4.3" />
            </svg>
            <input
              type="text"
              placeholder="搜索组件…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
        </div>
        <div className="divider" />
        <div
          style={{
            padding: "8px 20px",
            display: "flex",
            gap: 8,
            alignItems: "center",
            borderBottom: "1px solid var(--divider)",
          }}
        >
          {frameworks.map((f) => (
            <span
              key={f}
              className={`chip-filter${fwFilter === f ? " active" : ""}`}
              onClick={() => setFwFilter(f)}
            >
              {f}
            </span>
          ))}
        </div>
        <table className="table">
          <thead>
            <tr>
              <th style={{ width: 280 }}>组件</th>
              <th style={{ width: 120 }}>类型</th>
              <th style={{ width: 200 }}>路径</th>
              <th style={{ width: 100 }}>Props</th>
              <th style={{ width: 160 }}>标签</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((c) => (
              <tr key={c.id}>
                <td>
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <div
                      style={{
                        width: 30,
                        height: 30,
                        borderRadius: 8,
                        background: "rgba(99,102,241,0.08)",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        color: "var(--primary-light)",
                      }}
                    >
                      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
                        <rect x="3" y="7" width="18" height="13" rx="2" />
                        <path d="M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                      </svg>
                    </div>
                    <div>
                      <div style={{ fontSize: 13, fontWeight: 500, color: "var(--text-1)" }}>{c.name}</div>
                      <div style={{ fontSize: 11, color: "var(--text-3)" }}>
                        {c.description || "—"}
                      </div>
                    </div>
                  </div>
                </td>
                <td>
                  <span className="chip-type">{c.framework}</span>
                </td>
                <td>
                  <span className="mono" style={{ fontSize: 11, color: "var(--text-2)" }}>
                    {c.file_path}
                  </span>
                </td>
                <td style={{ color: "var(--text-mid)" }}>{c.props.length}</td>
                <td>
                  <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                    {c.tags.slice(0, 3).map((t) => (
                      <span key={t} className="chip-tag">
                        {t}
                      </span>
                    ))}
                  </div>
                </td>
                <td>
                  <Link to={`/components/${c.id}`} className="link-action">
                    详情
                  </Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {filtered.length === 0 && (
          <div className="empty">暂无组件，可上传文件或使用 AI 自然语言拉取</div>
        )}
      </div>

      <div className={`modal-backdrop${showUpload ? " open" : ""}`} onClick={() => setShowUpload(false)}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header">
            <div className="modal-title">上传组件</div>
            <div className="modal-sub">上传后本地解析，可选 AI 补全文档 / 示例 / Props 说明</div>
          </div>
          <form onSubmit={handleUpload}>
            <div className="modal-body">
              <div className="modal-field">
                <label>选择文件</label>
                <input
                  className="modal-input"
                  type="file"
                  multiple
                  required
                  accept=".vue,.uvue,.tsx,.jsx,.ts,.js,.md"
                  onChange={(e) => setFiles(e.target.files)}
                />
              </div>
              <label className="modal-check">
                <input type="checkbox" checked={useAi} onChange={(e) => setUseAi(e.target.checked)} />
                使用 AI 处理组件文件
              </label>
            </div>
            <div className="modal-footer">
              <button type="button" className="btn btn-ghost" onClick={() => setShowUpload(false)}>
                取消
              </button>
              <button type="submit" className="btn btn-primary" disabled={busy}>
                {busy ? "上传中…" : "上传并入库"}
              </button>
            </div>
          </form>
        </div>
      </div>

      <div className={`modal-backdrop${showFetch ? " open" : ""}`} onClick={() => setShowFetch(false)}>
        <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: 520 }}>
          <div className="modal-header">
            <div className="modal-title">AI 拉取</div>
            <div className="modal-sub">只用自然语言描述，AI 会定位、下载并解析组件</div>
          </div>
          <form onSubmit={handleFetch}>
            <div className="modal-body">
              <div className="modal-field">
                <label>需求描述</label>
                <textarea
                  className="modal-input"
                  required
                  value={fetchPrompt}
                  onChange={(e) => setFetchPrompt(e.target.value)}
                  placeholder="例如：从 Element Plus 拉取 Button 按钮组件的 Vue 源码"
                  style={{ minHeight: 100 }}
                />
                <div className="hint-text" style={{ marginTop: 6 }}>
                  {aiEnabled
                    ? "将调用 LLM 规划 raw 下载地址，下载后自动解析入库"
                    : "需先在「系统设置」配置 LLM"}
                </div>
              </div>
            </div>
            <div className="modal-footer">
              <button type="button" className="btn btn-ghost" onClick={() => setShowFetch(false)}>
                取消
              </button>
              <button type="submit" className="btn btn-primary" disabled={busy || !aiEnabled}>
                {busy ? "拉取中…" : "开始 AI 拉取"}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}
