import { FormEvent, useEffect, useState } from "react";
import { api, ApiKey } from "../api/client";

/**
 * API Keys admin page — secrets are stored server-side and always copyable.
 */
export default function ApiKeys() {
  const [keys, setKeys] = useState<ApiKey[]>([]);
  const [error, setError] = useState("");
  const [toast, setToast] = useState("");
  const [name, setName] = useState("");
  const [showModal, setShowModal] = useState(false);
  const [revealed, setRevealed] = useState<Record<string, boolean>>({});

  const load = () => {
    api.apiKeys
      .list()
      .then(setKeys)
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    load();
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(""), 2000);
    return () => clearTimeout(t);
  }, [toast]);

  /**
   * Copy text to clipboard and show a short toast.
   * @param {string} text
   * @param {string} [okMsg]
   */
  const copyText = async (text: string, okMsg = "已复制到剪贴板") => {
    try {
      await navigator.clipboard.writeText(text);
      setToast(okMsg);
    } catch {
      setError("复制失败，请手动选择文本");
    }
  };

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    try {
      const res = await api.apiKeys.create(name);
      setName("");
      setShowModal(false);
      load();
      if (res.key.key) {
        await copyText(res.key.key, "已创建并复制到剪贴板");
        setRevealed((m) => ({ ...m, [res.key.id]: true }));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
    }
  };

  const handleRegenerate = async (id: string) => {
    if (!confirm("重新生成后旧 Key 立即失效，确定？")) return;
    setError("");
    try {
      const res = await api.apiKeys.regenerate(id);
      load();
      if (res.key.key) {
        setRevealed((m) => ({ ...m, [id]: true }));
        await copyText(res.key.key, "新 Key 已生成并复制");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "重新生成失败");
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("确定删除此 API Key？")) return;
    try {
      await api.apiKeys.delete(id);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "删除失败");
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>API Keys</h1>
          <div className="page-sub">管理 MCP 客户端访问凭据（X-API-Key），可随时查看与复制</div>
        </div>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          + 创建 Key
        </button>
      </div>

      {error && <p className="error">{error}</p>}
      {toast && (
        <div className="tip-banner" style={{ borderColor: "rgba(52,211,153,0.4)" }}>
          <span>{toast}</span>
        </div>
      )}

      <div className="tip-banner">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="9" />
          <path d="M12 8v4" />
          <path d="M12 16h.01" />
        </svg>
        <span>
          在 Cursor 等 MCP 客户端中配置请求头：
          <code>X-API-Key: &lt;下方完整密钥&gt;</code>
        </span>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">API Keys</div>
          <span className="chip-count">{keys.length}</span>
        </div>
        <div className="divider" />
        {keys.length === 0 ? (
          <div className="empty">暂无 API Key</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 140 }}>名称</th>
                <th>完整密钥</th>
                <th style={{ width: 140 }}>创建时间</th>
                <th style={{ width: 140 }}>最近使用</th>
                <th style={{ width: 200 }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => {
                const secret = k.key?.trim() || "";
                const show = !!revealed[k.id];
                const display = secret
                  ? show
                    ? secret
                    : `${k.key_prefix}${"•".repeat(Math.max(8, secret.length - k.key_prefix.length))}`
                  : `${k.key_prefix}…（旧 Key 无法恢复，请重新生成）`;
                return (
                  <tr key={k.id}>
                    <td>
                      <span
                        className="mono"
                        style={{ fontSize: 12, fontWeight: 500, color: "var(--text-1)" }}
                      >
                        {k.name}
                      </span>
                    </td>
                    <td>
                      <div
                        className="mono"
                        style={{
                          fontSize: 11.5,
                          color: "var(--text-1)",
                          wordBreak: "break-all",
                          lineHeight: 1.45,
                          maxWidth: 420,
                        }}
                      >
                        {display}
                      </div>
                    </td>
                    <td style={{ fontSize: 11, color: "var(--text-2)" }}>
                      {new Date(k.created_at).toLocaleString()}
                    </td>
                    <td style={{ fontSize: 11, color: "var(--text-2)" }}>
                      {k.last_used_at ? new Date(k.last_used_at).toLocaleString() : "—"}
                    </td>
                    <td>
                      <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
                        {secret ? (
                          <>
                            <span
                              className="link-action"
                              onClick={() =>
                                setRevealed((m) => ({ ...m, [k.id]: !m[k.id] }))
                              }
                            >
                              {show ? "隐藏" : "显示"}
                            </span>
                            <span
                              className="link-action"
                              onClick={() => copyText(secret)}
                            >
                              复制
                            </span>
                          </>
                        ) : (
                          <span
                            className="link-action"
                            onClick={() => handleRegenerate(k.id)}
                          >
                            重新生成
                          </span>
                        )}
                        {secret && (
                          <span
                            className="link-action muted"
                            onClick={() => handleRegenerate(k.id)}
                          >
                            轮换
                          </span>
                        )}
                        <span
                          className="link-action danger"
                          onClick={() => handleDelete(k.id)}
                        >
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

      <div
        className={`modal-backdrop${showModal ? " open" : ""}`}
        onClick={() => setShowModal(false)}
      >
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header">
            <div className="modal-title">创建 API Key</div>
            <div className="modal-sub">创建后可随时在列表中查看与复制</div>
          </div>
          <form onSubmit={handleCreate}>
            <div className="modal-body">
              <div className="modal-field">
                <label>名称</label>
                <input
                  className="modal-input"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="例如：Cursor Agent"
                />
              </div>
            </div>
            <div className="modal-footer">
              <button type="button" className="btn btn-ghost" onClick={() => setShowModal(false)}>
                取消
              </button>
              <button type="submit" className="btn btn-primary">
                创建
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}
