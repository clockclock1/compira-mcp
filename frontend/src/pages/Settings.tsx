import { FormEvent, useEffect, useState } from "react";
import { api } from "../api/client";
import { useAuth } from "../auth/AuthContext";

interface LlmSettings {
  enabled: boolean;
  api_key_set: boolean;
  api_key_masked: string | null;
  base_url: string;
  model: string;
}

export default function Settings() {
  const { isAdmin } = useAuth();
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [error, setError] = useState("");
  const [ok, setOk] = useState("");
  const [saving, setSaving] = useState(false);

  const load = () => {
    api.settings
      .llm()
      .then((s) => {
        setSettings(s);
        setBaseUrl(s.base_url);
        setModel(s.model);
        setApiKey("");
      })
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    if (isAdmin) load();
  }, [isAdmin]);

  if (!isAdmin) {
    return <div className="card empty">需要管理员权限</div>;
  }

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setOk("");
    setSaving(true);
    try {
      const updated = await api.settings.updateLlm({
        api_key: apiKey.trim() || undefined,
        base_url: baseUrl.trim() || undefined,
        model: model.trim() || undefined,
      });
      setSettings(updated);
      setApiKey("");
      setBaseUrl(updated.base_url);
      setModel(updated.model);
      setOk("已保存，立即生效");
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSaving(false);
    }
  };

  const handleClearKey = async () => {
    if (!confirm("确定清除 API Key？AI 拉取与 AI 解析将不可用。")) return;
    setError("");
    setOk("");
    setSaving(true);
    try {
      const updated = await api.settings.updateLlm({ clear_api_key: true });
      setSettings(updated);
      setApiKey("");
      setOk("API Key 已清除");
    } catch (err) {
      setError(err instanceof Error ? err.message : "清除失败");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>系统设置</h1>
          <div className="page-sub">配置 LLM（OpenAI 兼容接口），供 AI 拉取与组件解析使用</div>
        </div>
        <div className={`chip-status ${settings?.enabled ? "chip-green" : ""}`}>
          <span className="dot" style={{ background: settings?.enabled ? undefined : "var(--text-4)" }} />
          {settings?.enabled ? "AI 已启用" : "AI 未配置"}
        </div>
      </div>

      {error && <p className="error">{error}</p>}
      {ok && <p style={{ color: "var(--green)", fontSize: 12.5, marginBottom: 12 }}>{ok}</p>}

      <div className="tip-banner">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="9" />
          <path d="M12 8v4" />
          <path d="M12 16h.01" />
        </svg>
        <span>
          支持任意 OpenAI 兼容 API。也可通过环境变量 <code>COMPIRA_LLM_*</code> 做首次种子配置；后台保存后以数据库为准。
        </span>
      </div>

      <div className="card" style={{ maxWidth: 640 }}>
        <div className="card-header">
          <div className="card-title">LLM 配置</div>
          {settings?.api_key_masked && (
            <span className="chip-file">{settings.api_key_masked}</span>
          )}
        </div>
        <div className="divider" />
        <form className="card-body" onSubmit={handleSave}>
          <div className="modal-field">
            <label>API Key</label>
            <input
              className="modal-input"
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={
                settings?.api_key_set
                  ? "已配置，留空表示不修改"
                  : "sk-xxxxxxxx"
              }
              autoComplete="off"
            />
            <div className="hint-text" style={{ marginTop: 6 }}>
              对应环境变量 COMPIRA_LLM_API_KEY / OPENAI_API_KEY
            </div>
          </div>
          <div className="modal-field" style={{ marginTop: 16 }}>
            <label>Base URL</label>
            <input
              className="modal-input"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.openai.com/v1"
              required
            />
            <div className="hint-text" style={{ marginTop: 6 }}>
              COMPIRA_LLM_BASE_URL，勿带尾斜杠以外的路径错误
            </div>
          </div>
          <div className="modal-field" style={{ marginTop: 16 }}>
            <label>Model</label>
            <input
              className="modal-input"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="gpt-4o-mini"
              required
            />
            <div className="hint-text" style={{ marginTop: 6 }}>
              COMPIRA_LLM_MODEL
            </div>
          </div>
          <div style={{ display: "flex", gap: 10, marginTop: 20 }}>
            <button type="submit" className="btn btn-primary" disabled={saving}>
              {saving ? "保存中…" : "保存设置"}
            </button>
            {settings?.api_key_set && (
              <button type="button" className="btn btn-ghost" onClick={handleClearKey} disabled={saving}>
                清除 API Key
              </button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
}
