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

interface SyncSettings {
  max_jobs: number;
  parse_concurrency: number;
  ingest_batch_size: number;
  download_concurrency: number;
}

export default function Settings() {
  const { isAdmin } = useAuth();
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [sync, setSync] = useState<SyncSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [maxJobs, setMaxJobs] = useState("2");
  const [parseConc, setParseConc] = useState("0");
  const [batchSize, setBatchSize] = useState("50");
  const [downloadConc, setDownloadConc] = useState("4");
  const [error, setError] = useState("");
  const [ok, setOk] = useState("");
  const [saving, setSaving] = useState(false);
  const [savingSync, setSavingSync] = useState(false);

  const load = () => {
    Promise.all([api.settings.llm(), api.settings.sync()])
      .then(([s, syncCfg]) => {
        setSettings(s);
        setBaseUrl(s.base_url);
        setModel(s.model);
        setApiKey("");
        setSync(syncCfg);
        setMaxJobs(String(syncCfg.max_jobs));
        setParseConc(String(syncCfg.parse_concurrency));
        setBatchSize(String(syncCfg.ingest_batch_size));
        setDownloadConc(String(syncCfg.download_concurrency));
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
      setOk("LLM 设置已保存，立即生效");
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSaving(false);
    }
  };

  const handleSaveSync = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setOk("");
    setSavingSync(true);
    try {
      const updated = await api.settings.updateSync({
        max_jobs: Math.max(1, Number(maxJobs) || 1),
        parse_concurrency: Math.max(0, Number(parseConc) || 0),
        ingest_batch_size: Math.max(1, Number(batchSize) || 50),
        download_concurrency: Math.max(1, Number(downloadConc) || 4),
      });
      setSync(updated);
      setMaxJobs(String(updated.max_jobs));
      setParseConc(String(updated.parse_concurrency));
      setBatchSize(String(updated.ingest_batch_size));
      setDownloadConc(String(updated.download_concurrency));
      setOk("同步并发已保存，立即生效（进行中的任务不受影响）");
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSavingSync(false);
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
    <div className="page settings-page">
      <div className="page-header">
        <div>
          <h1>系统设置</h1>
          <div className="page-sub">
            添加组件库不限速 · 仅同步/解析受并发限制
          </div>
        </div>
        <div className="settings-header-meta">
          <div className={`chip-status ${settings?.enabled ? "chip-green" : ""}`}>
            <span
              className="dot"
              style={{ background: settings?.enabled ? undefined : "var(--text-4)" }}
            />
            {settings?.enabled ? "AI 已启用" : "AI 未配置"}
          </div>
          {sync && (
            <span className="chip-file">
              并行 {sync.max_jobs} · 解析 {sync.parse_concurrency || "自动"}
            </span>
          )}
        </div>
      </div>

      {error && <p className="error">{error}</p>}
      {ok && <p className="settings-ok">{ok}</p>}

      <div className="settings-grid">
        {/* Sync concurrency */}
        <section className="card settings-card">
          <div className="card-header">
            <div>
              <div className="card-title">同步并发</div>
              <div className="settings-card-desc">
                控制同时跑几个库、解析与下载强度
              </div>
            </div>
          </div>
          <div className="divider" />
          <form className="card-body" onSubmit={handleSaveSync}>
            <div className="settings-fields-2">
              <div className="settings-field">
                <label htmlFor="sync-max-jobs">
                  同步并行数
                  <span className="settings-key">max_jobs</span>
                </label>
                <input
                  id="sync-max-jobs"
                  className="modal-input"
                  type="number"
                  min={1}
                  max={64}
                  value={maxJobs}
                  onChange={(e) => setMaxJobs(e.target.value)}
                  required
                />
                <p className="hint-text">同时同步/入库的库上限</p>
              </div>
              <div className="settings-field">
                <label htmlFor="sync-parse">
                  解析并发
                  <span className="settings-key">parse</span>
                </label>
                <input
                  id="sync-parse"
                  className="modal-input"
                  type="number"
                  min={0}
                  max={256}
                  value={parseConc}
                  onChange={(e) => setParseConc(e.target.value)}
                  required
                />
                <p className="hint-text">单库解析线程，0 = CPU 自动</p>
              </div>
              <div className="settings-field">
                <label htmlFor="sync-batch">
                  入库批次
                  <span className="settings-key">batch</span>
                </label>
                <input
                  id="sync-batch"
                  className="modal-input"
                  type="number"
                  min={1}
                  max={500}
                  value={batchSize}
                  onChange={(e) => setBatchSize(e.target.value)}
                  required
                />
                <p className="hint-text">每批写入 SQLite 的组件数</p>
              </div>
              <div className="settings-field">
                <label htmlFor="sync-download">
                  下载并发
                  <span className="settings-key">download</span>
                </label>
                <input
                  id="sync-download"
                  className="modal-input"
                  type="number"
                  min={1}
                  max={32}
                  value={downloadConc}
                  onChange={(e) => setDownloadConc(e.target.value)}
                  required
                />
                <p className="hint-text">AI 拉取时并行下载数</p>
              </div>
            </div>
            <div className="settings-actions">
              <button type="submit" className="btn btn-primary" disabled={savingSync}>
                {savingSync ? "保存中…" : "保存同步设置"}
              </button>
            </div>
          </form>
        </section>

        {/* LLM */}
        <section className="card settings-card">
          <div className="card-header">
            <div>
              <div className="card-title">LLM 配置</div>
              <div className="settings-card-desc">
                OpenAI 兼容接口，供 AI 拉取与解析
              </div>
            </div>
            {settings?.api_key_masked && (
              <span className="chip-file">{settings.api_key_masked}</span>
            )}
          </div>
          <div className="divider" />
          <form className="card-body" onSubmit={handleSave}>
            <div className="settings-field">
              <label htmlFor="llm-key">API Key</label>
              <input
                id="llm-key"
                className="modal-input"
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={
                  settings?.api_key_set ? "已配置，留空表示不修改" : "sk-xxxxxxxx"
                }
                autoComplete="off"
              />
              <p className="hint-text">COMPIRA_LLM_API_KEY / OPENAI_API_KEY</p>
            </div>
            <div className="settings-fields-2" style={{ marginTop: 14 }}>
              <div className="settings-field">
                <label htmlFor="llm-base">Base URL</label>
                <input
                  id="llm-base"
                  className="modal-input"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder="https://api.openai.com/v1"
                  required
                />
                <p className="hint-text">勿带错误路径后缀</p>
              </div>
              <div className="settings-field">
                <label htmlFor="llm-model">Model</label>
                <input
                  id="llm-model"
                  className="modal-input"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="gpt-4o-mini"
                  required
                />
                <p className="hint-text">COMPIRA_LLM_MODEL</p>
              </div>
            </div>
            <div className="settings-actions">
              <button type="submit" className="btn btn-primary" disabled={saving}>
                {saving ? "保存中…" : "保存 LLM"}
              </button>
              {settings?.api_key_set && (
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={handleClearKey}
                  disabled={saving}
                >
                  清除 API Key
                </button>
              )}
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}
