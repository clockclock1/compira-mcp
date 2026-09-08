import { FormEvent, useEffect, useState } from "react";
import { api } from "../api/client";
import { useAuth } from "../auth/AuthContext";
import MaintenancePanel from "../components/MaintenancePanel";

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
  fetch_max_attempts: number;
}

interface AuthSettings {
  session_ttl_hours: number;
  remember_me_ttl_hours: number;
  sliding: boolean;
}

function formatExpires(iso: string | null) {
  if (!iso) return "未知";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "未知";
  const left = d.getTime() - Date.now();
  if (left <= 0) return "已过期";
  const h = Math.floor(left / 3600_000);
  const m = Math.floor((left % 3600_000) / 60_000);
  if (h >= 48) return `${Math.floor(h / 24)} 天后 · ${d.toLocaleString()}`;
  if (h >= 1) return `${h} 小时 ${m} 分后 · ${d.toLocaleString()}`;
  return `${m} 分钟后 · ${d.toLocaleString()}`;
}

export default function Settings() {
  const { user, isAdmin, expiresAt, refreshUser } = useAuth();
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [sync, setSync] = useState<SyncSettings | null>(null);
  const [authCfg, setAuthCfg] = useState<AuthSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [maxJobs, setMaxJobs] = useState("2");
  const [parseConc, setParseConc] = useState("0");
  const [batchSize, setBatchSize] = useState("50");
  const [downloadConc, setDownloadConc] = useState("4");
  const [fetchAttempts, setFetchAttempts] = useState("4");
  const [sessionTtl, setSessionTtl] = useState("8");
  const [rememberTtl, setRememberTtl] = useState("168");
  const [sliding, setSliding] = useState(true);
  const [accountUser, setAccountUser] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState("");
  const [ok, setOk] = useState("");
  const [saving, setSaving] = useState(false);
  const [savingSync, setSavingSync] = useState(false);
  const [savingAuth, setSavingAuth] = useState(false);
  const [savingAccount, setSavingAccount] = useState(false);

  const loadAdmin = () => {
    if (!isAdmin) return;
    Promise.all([api.settings.llm(), api.settings.sync(), api.settings.auth()])
      .then(([s, syncCfg, a]) => {
        setSettings(s);
        setBaseUrl(s.base_url);
        setModel(s.model);
        setApiKey("");
        setSync(syncCfg);
        setMaxJobs(String(syncCfg.max_jobs));
        setParseConc(String(syncCfg.parse_concurrency));
        setBatchSize(String(syncCfg.ingest_batch_size));
        setDownloadConc(String(syncCfg.download_concurrency));
        setFetchAttempts(String(syncCfg.fetch_max_attempts ?? 4));
        setAuthCfg(a);
        setSessionTtl(String(a.session_ttl_hours));
        setRememberTtl(String(a.remember_me_ttl_hours));
        setSliding(a.sliding);
      })
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    if (user) setAccountUser(user.username);
  }, [user]);

  useEffect(() => {
    loadAdmin();
  }, [isAdmin]);

  const handleSaveAccount = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setOk("");
    if (!currentPassword) {
      setError("请输入当前密码以确认身份");
      return;
    }
    if (newPassword && newPassword.length < 6) {
      setError("新密码至少 6 位");
      return;
    }
    if (newPassword && newPassword !== confirmPassword) {
      setError("两次输入的新密码不一致");
      return;
    }
    setSavingAccount(true);
    try {
      await api.auth.updateAccount({
        username: accountUser.trim() || undefined,
        current_password: currentPassword,
        new_password: newPassword || undefined,
      });
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      await refreshUser();
      setOk(
        newPassword
          ? "账号已更新；其他设备的登录已失效"
          : "用户名已更新",
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSavingAccount(false);
    }
  };

  const handleSaveAuth = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setOk("");
    setSavingAuth(true);
    try {
      const updated = await api.settings.updateAuth({
        session_ttl_hours: Math.max(1, Number(sessionTtl) || 8),
        remember_me_ttl_hours: Math.max(1, Number(rememberTtl) || 168),
        sliding,
      });
      setAuthCfg(updated);
      setSessionTtl(String(updated.session_ttl_hours));
      setRememberTtl(String(updated.remember_me_ttl_hours));
      setSliding(updated.sliding);
      setOk("登录时效已保存，对新登录立即生效");
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSavingAuth(false);
    }
  };

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
        fetch_max_attempts: Math.min(12, Math.max(1, Number(fetchAttempts) || 4)),
      });
      setSync(updated);
      setMaxJobs(String(updated.max_jobs));
      setParseConc(String(updated.parse_concurrency));
      setBatchSize(String(updated.ingest_batch_size));
      setDownloadConc(String(updated.download_concurrency));
      setFetchAttempts(String(updated.fetch_max_attempts));
      setOk("同步并发已保存，立即生效（进行中的任务不受影响）");
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSavingSync(false);
    }
  };

  const handleClearKey = async () => {
    if (!confirm("确定清除 API Key？AI 拉取与自动命名将不可用。")) return;
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
          <div className="page-sub">账号密码 · 登录时效 · 存储维护 · 同步拉取 · LLM</div>
        </div>
        <div className="settings-header-meta">
          {user && <span className="chip-file">{user.username}</span>}
          <div className={`chip-status ${settings?.enabled ? "chip-green" : ""}`}>
            <span
              className="dot"
              style={{
                background: isAdmin
                  ? settings?.enabled
                    ? undefined
                    : "var(--text-4)"
                  : "var(--text-4)",
              }}
            />
            {isAdmin
              ? settings?.enabled
                ? "AI 已启用"
                : "AI 未配置"
              : "普通用户"}
          </div>
        </div>
      </div>

      {error && <p className="error">{error}</p>}
      {ok && <p className="settings-ok">{ok}</p>}

      {isAdmin && (
        <div style={{ marginBottom: 18 }}>
          <MaintenancePanel
            onError={(msg) => {
              setOk("");
              setError(msg);
            }}
            onOk={(msg) => {
              setError("");
              setOk(msg);
            }}
          />
        </div>
      )}

      <div className="settings-grid">
        {/* Account — all users */}
        <section className="card settings-card">
          <div className="card-header">
            <div>
              <div className="card-title">账号与密码</div>
              <div className="settings-card-desc">
                修改登录用户名或密码 · 当前会话约 {formatExpires(expiresAt)}
              </div>
            </div>
          </div>
          <div className="divider" />
          <form className="card-body" onSubmit={handleSaveAccount}>
            <div className="settings-field">
              <label htmlFor="acc-user">用户名</label>
              <input
                id="acc-user"
                className="modal-input"
                value={accountUser}
                onChange={(e) => setAccountUser(e.target.value)}
                minLength={3}
                required
                autoComplete="username"
              />
            </div>
            <div className="settings-field" style={{ marginTop: 14 }}>
              <label htmlFor="acc-cur">当前密码</label>
              <input
                id="acc-cur"
                className="modal-input"
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                required
                autoComplete="current-password"
                placeholder="确认身份必填"
              />
            </div>
            <div className="settings-fields-2" style={{ marginTop: 14 }}>
              <div className="settings-field">
                <label htmlFor="acc-new">新密码</label>
                <input
                  id="acc-new"
                  className="modal-input"
                  type="password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  minLength={6}
                  autoComplete="new-password"
                  placeholder="留空则不改密码"
                />
              </div>
              <div className="settings-field">
                <label htmlFor="acc-confirm">确认新密码</label>
                <input
                  id="acc-confirm"
                  className="modal-input"
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  autoComplete="new-password"
                  placeholder="再次输入新密码"
                />
              </div>
            </div>
            <div className="settings-actions">
              <button type="submit" className="btn btn-primary" disabled={savingAccount}>
                {savingAccount ? "保存中…" : "保存账号"}
              </button>
            </div>
          </form>
        </section>

        {/* Session policy — admin */}
        {isAdmin && (
          <section className="card settings-card">
            <div className="card-header">
              <div>
                <div className="card-title">登录时效</div>
                <div className="settings-card-desc">
                  控制会话多久过期；勾选「记住我」使用较长有效期
                </div>
              </div>
              {authCfg && (
                <span className="chip-file">
                  {authCfg.session_ttl_hours}h / 记住 {authCfg.remember_me_ttl_hours}h
                  {authCfg.sliding ? " · 滑动续期" : ""}
                </span>
              )}
            </div>
            <div className="divider" />
            <form className="card-body" onSubmit={handleSaveAuth}>
              <div className="settings-fields-2">
                <div className="settings-field">
                  <label htmlFor="auth-ttl">普通登录有效期（小时）</label>
                  <input
                    id="auth-ttl"
                    className="modal-input"
                    type="number"
                    min={1}
                    max={2160}
                    value={sessionTtl}
                    onChange={(e) => setSessionTtl(e.target.value)}
                    required
                  />
                  <p className="hint-text">未勾选「记住我」时的时长，默认 8 小时</p>
                </div>
                <div className="settings-field">
                  <label htmlFor="auth-remember">记住我有效期（小时）</label>
                  <input
                    id="auth-remember"
                    className="modal-input"
                    type="number"
                    min={1}
                    max={8760}
                    value={rememberTtl}
                    onChange={(e) => setRememberTtl(e.target.value)}
                    required
                  />
                  <p className="hint-text">勾选「记住我」时，默认 168 小时（7 天）</p>
                </div>
              </div>
              <label
                className="settings-check"
                style={{ marginTop: 14, display: "flex", alignItems: "center", gap: 10 }}
              >
                <input
                  type="checkbox"
                  checked={sliding}
                  onChange={(e) => setSliding(e.target.checked)}
                />
                <span>
                  滑动续期
                  <span className="hint-text" style={{ display: "block", marginTop: 4 }}>
                    有操作时自动延长会话，避免中途突然掉线
                  </span>
                </span>
              </label>
              <div className="settings-actions">
                <button type="submit" className="btn btn-primary" disabled={savingAuth}>
                  {savingAuth ? "保存中…" : "保存登录时效"}
                </button>
              </div>
            </form>
          </section>
        )}

        {isAdmin && (
          <section className="card settings-card">
            <div className="card-header">
              <div>
                <div className="card-title">同步与拉取</div>
                <div className="settings-card-desc">
                  控制同步并行度，以及 AI 拉取换源重试次数
                </div>
              </div>
              {sync && (
                <span className="chip-file">
                  并行 {sync.max_jobs} · 拉取尝试 {sync.fetch_max_attempts ?? 4}
                </span>
              )}
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
                <div className="settings-field">
                  <label htmlFor="sync-fetch-attempts">
                    AI 拉取最大尝试次数
                    <span className="settings-key">fetch_attempts</span>
                  </label>
                  <input
                    id="sync-fetch-attempts"
                    className="modal-input"
                    type="number"
                    min={1}
                    max={12}
                    value={fetchAttempts}
                    onChange={(e) => setFetchAttempts(e.target.value)}
                    required
                  />
                  <p className="hint-text">
                    下载失败后让 AI 另找链接的次数，1–12（不自动换镜像）
                  </p>
                </div>
              </div>
              <div className="settings-actions">
                <button type="submit" className="btn btn-primary" disabled={savingSync}>
                  {savingSync ? "保存中…" : "保存同步设置"}
                </button>
              </div>
            </form>
          </section>
        )}

        {isAdmin && (
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
        )}
      </div>
    </div>
  );
}
