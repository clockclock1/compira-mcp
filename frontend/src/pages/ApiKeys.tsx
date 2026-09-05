import { FormEvent, useEffect, useState } from "react";
import { api, ApiKey } from "../api/client";

export default function ApiKeys() {
  const [keys, setKeys] = useState<ApiKey[]>([]);
  const [error, setError] = useState("");
  const [newKey, setNewKey] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [showModal, setShowModal] = useState(false);

  const load = () => {
    api.apiKeys
      .list()
      .then(setKeys)
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    load();
  }, []);

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    try {
      const res = await api.apiKeys.create(name);
      setNewKey(res.key.key);
      setName("");
      setShowModal(false);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
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
          <div className="page-sub">管理 MCP 客户端访问凭据（X-API-Key）</div>
        </div>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          + 创建 Key
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      <div className="tip-banner">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="9" />
          <path d="M12 8v4" />
          <path d="M12 16h.01" />
        </svg>
        <span>
          在 Cursor 等 MCP 客户端中配置：X-API-Key: <code>cmcp_xxxxxxxx</code>
          ，即可访问组件库索引、搜索与元数据接口
        </span>
      </div>

      {newKey && (
        <div className="card" style={{ marginBottom: 20, borderColor: "rgba(52,211,153,0.35)" }}>
          <div className="card-header">
            <div className="card-title">新 Key 已创建（仅显示一次）</div>
          </div>
          <div className="divider" />
          <div className="card-body">
            <pre className="code-block" style={{ wordBreak: "break-all" }}>
              {newKey}
            </pre>
            <button
              className="btn btn-primary"
              style={{ marginTop: 12 }}
              onClick={() => {
                navigator.clipboard?.writeText(newKey).catch(() => {});
                setNewKey(null);
              }}
            >
              复制并关闭
            </button>
          </div>
        </div>
      )}

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
                <th style={{ width: 180 }}>名称</th>
                <th style={{ width: 280 }}>前缀</th>
                <th style={{ width: 150 }}>创建时间</th>
                <th style={{ width: 150 }}>最近使用</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={k.id}>
                  <td>
                    <span className="mono" style={{ fontSize: 12, fontWeight: 500, color: "var(--text-1)" }}>
                      {k.name}
                    </span>
                  </td>
                  <td>
                    <span className="mono" style={{ fontSize: 11, color: "var(--text-mid)" }}>
                      {k.key_prefix}…
                    </span>
                  </td>
                  <td style={{ fontSize: 11, color: "var(--text-2)" }}>
                    {new Date(k.created_at).toLocaleString()}
                  </td>
                  <td style={{ fontSize: 11, color: "var(--text-2)" }}>
                    {k.last_used_at ? new Date(k.last_used_at).toLocaleString() : "—"}
                  </td>
                  <td>
                    <span className="link-action danger" onClick={() => handleDelete(k.id)}>
                      删除
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className={`modal-backdrop${showModal ? " open" : ""}`} onClick={() => setShowModal(false)}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header">
            <div className="modal-title">创建 API Key</div>
            <div className="modal-sub">用于 MCP 客户端鉴权，创建后仅显示一次完整密钥</div>
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
