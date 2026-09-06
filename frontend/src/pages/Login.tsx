import { FormEvent, useState } from "react";
import { Navigate } from "react-router-dom";
import { useAuth } from "../auth/AuthContext";
import AlertBanner from "../components/AlertBanner";
import { AmbientOrbs, GrainOverlay, ShineButton } from "../components/motion";

function LogoIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 17l6-6-6-6" />
      <path d="M12 19h8" />
    </svg>
  );
}

export default function Login() {
  const { user, login, loading } = useAuth();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [remember, setRemember] = useState(true);

  if (!loading && user) {
    return <Navigate to="/" replace />;
  }

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setSubmitting(true);
    try {
      await login(username, password, remember);
    } catch (err) {
      setError(err instanceof Error ? err.message : "登录失败");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div id="login-view" className="login-lux">
      <AmbientOrbs />
      <GrainOverlay />
      <div className="login-grid-bg" aria-hidden />
      <div className="login-top-logo login-top-logo-anim">
        <div className="login-logo-icon logo-pulse">
          <LogoIcon />
        </div>
        <div>
          <div className="login-logo-title brand-display">CompiraMCP</div>
          <div className="login-logo-sub">COMPONENT LIBRARY SERVER</div>
        </div>
      </div>
      <div className="login-card login-card-lux glass-surface">
        <div className="login-card-shine" aria-hidden />
        <div className="login-card-title brand-display">欢迎回来</div>
        <div className="login-card-sub">登录控制台 · 管理组件库 · 为 Agent 提供 MCP 检索</div>
        <form onSubmit={handleSubmit}>
          <div className="login-field field-reveal" style={{ ["--d" as string]: "0.08s" }}>
            <label htmlFor="login-user">用户名</label>
            <input
              className="login-input lux-input"
              id="login-user"
              type="text"
              autoFocus
              required
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="用户名"
            />
          </div>
          <div className="login-field field-reveal" style={{ marginTop: 16, ["--d" as string]: "0.16s" }}>
            <label htmlFor="login-pass">密码</label>
            <input
              className="login-input lux-input"
              id="login-pass"
              type="password"
              required
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••••"
            />
          </div>
          <div className="login-options field-reveal" style={{ ["--d" as string]: "0.22s" }}>
            <label className="remember" onClick={() => setRemember(!remember)}>
              <span
                className="checkbox"
                style={{
                  background: remember ? "var(--accent)" : "var(--bg-input)",
                  border: remember ? "none" : "1px solid var(--border)",
                }}
              >
                {remember && (
                  <svg viewBox="0 0 10 10">
                    <polyline points="1,5.2 3.8,8 9,2" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                )}
              </span>
              记住我（延长登录有效期）
            </label>
          </div>
          {error && (
            <div style={{ marginTop: 14 }} className="field-reveal" >
              <AlertBanner title="登录失败" message={error} onClose={() => setError("")} />
            </div>
          )}
          <div className="field-reveal" style={{ ["--d" as string]: "0.28s" }}>
            <ShineButton type="submit" disabled={submitting} className="btn btn-primary shine-btn login-btn-lux">
              {submitting ? "登录中..." : "进入控制台"}
            </ShineButton>
          </div>
        </form>
        <div className="login-note">账号可在「系统设置」修改；会话到期后需重新登录</div>
      </div>
      <div className="login-copyright">Copyright 2026 CompiraMCP</div>
      <div className="login-version chip-glow">v0.1.0</div>
    </div>
  );
}
