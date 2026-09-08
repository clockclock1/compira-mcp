import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { api, Library, SyncTask } from "../api/client";
import AlertBanner from "../components/AlertBanner";
import EmptyState from "../components/EmptyState";
import Modal from "../components/Modal";
import { PageMotion, ShineButton } from "../components/motion";

type Filter = "all" | "synced" | "syncing" | "failed";
type CreateMode = "git" | "upload" | "fetch";

function mapStatus(status: string): Filter {
  if (status === "ready") return "synced";
  if (status === "error") return "failed";
  if (status === "syncing") return "syncing";
  return "all";
}

function statusBadge(status: string, task?: SyncTask) {
  if (task) return <span className="badge-sync badge-syncing">同步中</span>;
  if (status === "ready") return <span className="badge-sync badge-ok">已同步</span>;
  if (status === "error") return <span className="badge-sync badge-fail">失败</span>;
  if (status === "syncing") return <span className="badge-sync badge-syncing">同步中</span>;
  return <span className="badge-sync badge-idle">未同步</span>;
}

function formatTime(iso: string | null) {
  if (!iso) return "未同步";
  return new Date(iso).toLocaleString();
}

function sourceLabel(lib: Library) {
  const t = lib.source_type || "git";
  if (t === "upload") return "上传";
  if (t === "fetch") return "AI 拉取";
  return "Git";
}

function shortRepo(url: string) {
  if (url.startsWith("upload://") || url.startsWith("fetch://")) return "本地入库";
  return url.replace(/^https?:\/\//, "").replace(/\.git$/, "");
}

export default function Libraries() {
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [tasks, setTasks] = useState<Record<string, SyncTask>>({});
  const [error, setError] = useState("");
  const [showModal, setShowModal] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [keyword, setKeyword] = useState("");
  const [mode, setMode] = useState<CreateMode>("git");
  const [aiEnabled, setAiEnabled] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [form, setForm] = useState({
    name: "",
    repo_url: "",
    branch: "main",
    rules: "",
    prompt: "",
  });
  const [files, setFiles] = useState<FileList | null>(null);
  const alertRef = useRef<HTMLDivElement>(null);
  const seenFailedTasks = useRef<Set<string>>(new Set());
  const watchedTasks = useRef<Set<string>>(new Set());
  const libNameById = useRef<Record<string, string>>({});

  const load = useCallback(() => {
    api.libraries
      .list()
      .then((list) => {
        setLibraries(list);
        const map: Record<string, string> = {};
        for (const l of list) map[l.id] = l.name;
        libNameById.current = map;
      })
      .catch((e) => setError(e.message));
  }, []);

  useEffect(() => {
    load();
    api.ai.status().then((s) => setAiEnabled(s.enabled)).catch(() => setAiEnabled(false));
  }, [load]);

  useEffect(() => {
    let wasRunning = false;
    const interval = setInterval(() => {
      api.tasks
        .list()
        .then((allTasks) => {
          const running = allTasks.filter((t) => t.status === "running" || t.status === "pending");
          const map: Record<string, SyncTask> = {};
          for (const t of running) {
            map[t.library_id] = t;
            watchedTasks.current.add(t.id);
          }
          setTasks(map);
          // Live counts while jobs run; one more refresh when the last job finishes.
          if (running.length > 0 || wasRunning) load();
          wasRunning = running.length > 0;

          for (const t of allTasks) {
            if (t.status !== "failed") continue;
            if (!watchedTasks.current.has(t.id) || seenFailedTasks.current.has(t.id)) continue;
            seenFailedTasks.current.add(t.id);
            const libName = libNameById.current[t.library_id];
            const prefix = libName ? `「${libName}」` : "任务";
            setError(`${prefix}失败：${t.message || "未知错误"}`);
          }
        })
        .catch(() => {});
    }, 2000);
    return () => clearInterval(interval);
  }, [load]);

  useEffect(() => {
    if (error) alertRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [error]);

  const failedLibs = useMemo(
    () => libraries.filter((l) => l.status === "error" && l.last_error),
    [libraries],
  );

  const filtered = useMemo(() => {
    return libraries.filter((l) => {
      const mapped = mapStatus(l.status);
      const statusOk =
        filter === "all" ||
        (filter === "syncing" && (mapped === "syncing" || !!tasks[l.id])) ||
        (filter !== "syncing" && mapped === filter);
      const kw = keyword.trim().toLowerCase();
      const kwOk =
        !kw ||
        l.name.toLowerCase().includes(kw) ||
        l.repo_url.toLowerCase().includes(kw);
      return statusOk && kwOk;
    });
  }, [libraries, filter, keyword, tasks]);

  const resetForm = () => {
    setForm({
      name: "",
      repo_url: "",
      branch: "main",
      rules: "",
      prompt: "",
    });
    setFiles(null);
    setMode("git");
  };

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setSubmitting(true);
    try {
      if (mode === "git") {
        await api.libraries.create({
          name: form.name || form.repo_url.split("/").pop()?.replace(/\.git$/, "") || "library",
          repo_url: form.repo_url,
          branch: form.branch || "main",
          rules: form.rules || undefined,
          source_type: "git",
        });
      } else if (mode === "upload") {
        const autoName = !form.name.trim();
        const lib = await api.libraries.create({
          name: form.name.trim() || "处理中…",
          source_type: "upload",
          rules: form.rules || undefined,
        });
        if (files && files.length > 0) {
          await api.libraries.upload(lib.id, Array.from(files), autoName);
        }
      } else {
        if (!form.prompt.trim()) {
          throw new Error("请填写需求描述或仓库地址");
        }
        const prompt = form.prompt.trim();
        const repoOnly =
          /https?:\/\/(www\.)?(github|gitlab|gitee)\.com\//i.test(prompt) &&
          prompt.replace(/https?:\/\/\S+/gi, "").trim().length < 2;
        if (!aiEnabled && !repoOnly) {
          throw new Error("按组件 AI 拉取需配置 LLM；仅贴仓库地址可整库导入");
        }
        const autoName = !form.name.trim();
        const lib = await api.libraries.create({
          name: form.name.trim() || "处理中…",
          source_type: "fetch",
          rules: form.rules || undefined,
        });
        await api.libraries.fetch(lib.id, {
          prompt,
          auto_name: autoName,
        });
      }
      setShowModal(false);
      resetForm();
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
    } finally {
      setSubmitting(false);
    }
  };

  const handleSync = async (lib: Library) => {
    if (tasks[lib.id]) return;
    setError("");
    try {
      await api.libraries.sync(lib.id);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "同步失败");
    }
  };

  const handleCancel = async (lib: Library) => {
    setError("");
    try {
      await api.libraries.cancel(lib.id);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "中断失败");
    }
  };

  const handleSyncAll = async () => {
    setError("");
    const targets = libraries.filter((l) => !tasks[l.id]);
    if (targets.length === 0) {
      setError("没有可同步的组件库（可能都在进行中）");
      return;
    }
    let ok = 0;
    const errors: string[] = [];
    for (const lib of targets) {
      try {
        await api.libraries.sync(lib.id);
        ok += 1;
      } catch (err) {
        errors.push(`${lib.name}: ${err instanceof Error ? err.message : "失败"}`);
      }
    }
    load();
    if (errors.length) {
      setError(`已启动 ${ok} 个同步；失败：${errors.join("；")}`);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("确定删除此组件库？源码和索引将被清除。")) return;
    try {
      await api.libraries.delete(id);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "删除失败");
    }
  };

  return (
    <PageMotion>
      <div className="page-header">
        <div>
          <h1>组件库</h1>
          <div className="page-sub">
            可随时批量添加；同步并行与解析并发在「系统设置」配置
            {aiEnabled ? " · AI 已启用" : " · AI 未配置（管理员可在「系统设置」配置 LLM）"}
          </div>
        </div>
        <div className="header-actions">
          <div className="search-box">
            <svg viewBox="0 0 24 24">
              <circle cx="11" cy="11" r="7" />
              <path d="M21 21l-4.3-4.3" />
            </svg>
            <input
              type="text"
              placeholder="搜索仓库名称…"
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
            />
          </div>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={handleSyncAll}
            disabled={libraries.length === 0}
            title="一键同步全部组件库（并行数见系统设置）"
          >
            <svg viewBox="0 0 24 24">
              <path d="M21 12a9 9 0 1 1-2.6-6.4" />
              <path d="M21 3v6h-6" />
            </svg>
            一键同步
          </button>
          <ShineButton onClick={() => setShowModal(true)}>
            <svg viewBox="0 0 24 24">
              <path d="M12 5v14" />
              <path d="M5 12h14" />
            </svg>
            添加组件库
          </ShineButton>
        </div>
      </div>

      <div ref={alertRef}>
        {error && (
          <AlertBanner title="操作失败" message={error} onClose={() => setError("")} />
        )}
        {!error && failedLibs.length > 0 && (
          <div className="alert-banner alert-error" role="alert">
            <div className="alert-banner-icon" aria-hidden>
              <svg viewBox="0 0 24 24" width="20" height="20">
                <circle cx="12" cy="12" r="10" />
                <path d="M12 8v5" />
                <path d="M12 16h.01" />
              </svg>
            </div>
            <div className="alert-banner-body">
              <div className="alert-banner-title">
                {failedLibs.length} 个组件库同步/入库失败
              </div>
              <ul className="alert-banner-list">
                {failedLibs.map((lib) => (
                  <li key={lib.id}>
                    <div className="lib-name">
                      <Link to={`/libraries/${lib.id}`} className="link-action">
                        {lib.name}
                      </Link>
                    </div>
                    <div className="lib-err">{lib.last_error}</div>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        )}
      </div>

      <div className="card">
        <div className="card-header" style={{ padding: "14px 20px" }}>
          <span style={{ fontSize: 12.5, color: "var(--text-2)" }}>
            共 <span>{libraries.length}</span> 个仓库
          </span>
          <span style={{ flex: 1 }} />
          <div className="filter-chips">
            {(
              [
                ["all", "全部"],
                ["synced", "已同步"],
                ["syncing", "同步中"],
                ["failed", "失败"],
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
        </div>
        <div className="divider" />
        {filtered.length === 0 ? (
          <EmptyState
            title={libraries.length === 0 ? "暂无组件库" : "没有匹配的结果"}
            description={
              libraries.length === 0
                ? "支持 Git 同步、上传组件文件，或用自然语言 AI 拉取整库"
                : "试试切换筛选条件或清空搜索关键词"
            }
            actionLabel={libraries.length === 0 ? "添加组件库" : undefined}
            onAction={libraries.length === 0 ? () => setShowModal(true) : undefined}
          />
        ) : (
          <table className="table table-row-lg">
            <thead>
              <tr>
                <th style={{ width: 300 }}>仓库</th>
                <th style={{ width: 100 }}>来源</th>
                <th style={{ width: 100 }}>分支</th>
                <th style={{ width: 120 }}>状态</th>
                <th style={{ width: 90 }}>组件数</th>
                <th style={{ width: 180 }}>最近同步</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((lib) => {
                const task = tasks[lib.id];
                return (
                  <tr key={lib.id}>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                        <div
                          style={{
                            width: 30,
                            height: 30,
                            borderRadius: 8,
                            background:
                              lib.status === "error"
                                ? "rgba(248,113,113,0.10)"
                                : "rgba(99,102,241,0.08)",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            color:
                              lib.status === "error" ? "var(--red)" : "var(--primary-light)",
                          }}
                        >
                          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
                            <path d="M3 7l9-4 9 4-9 4-9-4z" />
                            <path d="M3 7v10l9 4 9-4V7" />
                          </svg>
                        </div>
                        <div>
                          <div style={{ fontSize: 13, fontWeight: 500, color: "var(--text-1)" }}>
                            {lib.name}
                          </div>
                          <div style={{ fontSize: 11, color: "var(--text-3)" }}>
                            {shortRepo(lib.repo_url)}
                          </div>
                        </div>
                      </div>
                    </td>
                    <td>
                      <span className="chip-tag">{sourceLabel(lib)}</span>
                    </td>
                    <td>
                      <span className="mono" style={{ fontSize: 12, color: "var(--text-2)" }}>
                        {lib.branch}
                      </span>
                    </td>
                    <td>
                      {statusBadge(lib.status, task)}
                      {task && (
                        <div>
                          <div
                            style={{
                              fontSize: 11,
                              color: task.status === "failed" ? "var(--red)" : "var(--text-3)",
                              marginTop: 4,
                              maxWidth: 220,
                              wordBreak: "break-word",
                            }}
                            title={task.message}
                          >
                            {task.message}
                          </div>
                          {task.status !== "failed" && (
                            <div className="progress-bar">
                              <div className="progress-bar-fill" style={{ width: `${task.progress}%` }} />
                            </div>
                          )}
                        </div>
                      )}
                      {!task && lib.status === "error" && lib.last_error && (
                        <div
                          style={{
                            fontSize: 11,
                            color: "var(--red)",
                            marginTop: 4,
                            maxWidth: 220,
                            wordBreak: "break-word",
                          }}
                          title={lib.last_error}
                        >
                          {lib.last_error}
                        </div>
                      )}
                    </td>
                    <td style={{ fontFamily: "var(--font-en)", color: "var(--text-mid)" }}>
                      {lib.component_count}
                    </td>
                    <td style={{ fontSize: 12, color: "var(--text-2)" }}>
                      {formatTime(lib.last_synced_at)}
                    </td>
                    <td>
                      <div className="row-actions">
                        <Link to={`/libraries/${lib.id}`} className="link-action">
                          详情
                        </Link>
                        {task ? (
                          <button
                            type="button"
                            className="btn-sync"
                            onClick={() => handleCancel(lib)}
                            title="中断当前同步，已入库组件会保留"
                            style={{ color: "var(--red)" }}
                          >
                            中断
                          </button>
                        ) : (
                          <button
                            type="button"
                            className="btn-sync"
                            onClick={() => handleSync(lib)}
                            title={
                              (lib.source_type || "git") === "git"
                                ? "拉取远程并重新索引"
                                : "重新扫描本地文件并索引"
                            }
                          >
                            <svg viewBox="0 0 24 24" width="13" height="13">
                              <path d="M21 12a9 9 0 1 1-2.6-6.4" />
                              <path d="M21 3v6h-6" />
                            </svg>
                            同步
                          </button>
                        )}
                        <span className="link-action danger" onClick={() => handleDelete(lib.id)}>
                          删除
                        </span>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      <Modal open={showModal} onClose={() => setShowModal(false)} width={520}>
          <div className="modal-header">
            <div className="modal-title">添加组件库</div>
            <div className="modal-sub">选择 Git 同步、上传文件，或用自然语言让 AI 拉取组件</div>
          </div>
          <form onSubmit={handleCreate}>
            <div className="modal-body">
              <div className="filter-chips" style={{ marginBottom: 4 }}>
                {(
                  [
                    ["git", "Git 仓库"],
                    ["upload", "上传文件"],
                    ["fetch", "AI 拉取"],
                  ] as const
                ).map(([key, label]) => (
                  <span
                    key={key}
                    className={`chip-filter${mode === key ? " active" : ""}`}
                    onClick={() => setMode(key)}
                  >
                    {label}
                  </span>
                ))}
              </div>

              <div className="modal-field">
                <label>名称{mode !== "git" ? "（可选）" : ""}</label>
                <input
                  className="modal-input"
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  placeholder={
                    mode === "git"
                      ? "可选，默认取仓库名"
                      : "可选，留空由 AI 命名"
                  }
                />
              </div>

              {mode === "git" && (
                <>
                  <div className="modal-field">
                    <label>仓库地址</label>
                    <input
                      className="modal-input"
                      required
                      value={form.repo_url}
                      onChange={(e) => setForm({ ...form, repo_url: e.target.value })}
                      placeholder="https://github.com/org/repo.git"
                    />
                  </div>
                  <div className="modal-field">
                    <label>分支</label>
                    <input
                      className="modal-input"
                      value={form.branch}
                      onChange={(e) => setForm({ ...form, branch: e.target.value })}
                    />
                  </div>
                </>
              )}

              {mode === "upload" && (
                <div className="modal-field">
                  <label>组件文件或压缩包（.zip）</label>
                  <input
                    className="modal-input"
                    type="file"
                    multiple
                    accept=".vue,.uvue,.tsx,.jsx,.ts,.js,.svelte,.astro,.dart,.wxml,.html,.css,.scss,.wxss,.json,.md,.zip,application/zip"
                    onChange={(e) => setFiles(e.target.files)}
                  />
                  <div className="hint-text" style={{ marginTop: 6 }}>
                    支持 Vue/React/Solid/Taro、Svelte、Astro、Angular(*.component.ts)、Lit/Web
                    Components、小程序(.wxml)、Flutter(.dart) 等；可多选或上传 .zip
                  </div>
                </div>
              )}

              {mode === "fetch" && (
                <div className="modal-field">
                  <label>用自然语言描述要拉取的组件</label>
                  <textarea
                    className="modal-input"
                    required
                    value={form.prompt}
                    onChange={(e) => setForm({ ...form, prompt: e.target.value })}
                    placeholder="例如：从 Element Plus 拉取 Button；或只贴仓库地址 https://github.com/xxx/yyy 整库导入"
                    style={{ minHeight: 100 }}
                  />
                  <div className="hint-text" style={{ marginTop: 6 }}>
                    指定组件时由 AI 定位 raw 文件；若只给仓库地址（或加「全部组件」），会克隆仓库并批量导入组件源文件
                    （Vue/React/Svelte/Astro/Angular/小程序/Flutter 等）
                    {!aiEnabled && "（整库导入可不配 LLM；按组件拉取需配置）"}
                  </div>
                </div>
              )}

              {mode === "upload" && (
                <div className="hint-text">
                  入库仅本地解析 Props/Events 等；名称留空时可由 AI 自动命名
                  {!aiEnabled && "（自动命名需配置 LLM）"}
                </div>
              )}
            </div>
            <div className="modal-footer">
              <button type="button" className="btn btn-ghost" onClick={() => setShowModal(false)}>
                取消
              </button>
              <button type="submit" className="btn btn-primary" disabled={submitting}>
                {submitting ? "处理中…" : mode === "git" ? "创建" : "创建并入库"}
              </button>
            </div>
          </form>
      </Modal>
    </PageMotion>
  );
}
