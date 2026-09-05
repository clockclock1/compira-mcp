import { FormEvent, useState } from "react";
import { Navigate } from "react-router-dom";
import { useAuth } from "../auth/AuthContext";

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
  const [username, setUsername] = useState("admin");
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
      await login(username, password);
    } catch (err) {
      setError(err instanceof Error ? err.message : "登录失败");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div id="login-view">
      <div className="login-glow glow-indigo" />
      <div className="login-glow glow-violet" />
      <div className="login-top-logo">
        <div className="login-logo-icon">
          <LogoIcon />
        </div>
        <div>
          <div className="login-logo-title">组件库 MCP</div>
          <div className="login-logo-sub">COMPONENT LIBRARY SERVER</div>
        </div>
      </div>
      <div className="login-card">
        <div className="login-card-title">欢迎回来</div>
        <div className="login-card-sub">登录组件库 MCP 服务器控制台，统一 Bearer Token 鉴权</div>
        <form onSubmit={handleSubmit}>
          <div className="login-field">
            <label htmlFor="login-user">用户名</label>
            <input
              className="login-input"
              id="login-user"
              type="text"
              autoFocus
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="admin"
            />
          </div>
          <div className="login-field" style={{ marginTop: 16 }}>
            <label htmlFor="login-pass">密码</label>
            <input
              className="login-input"
              id="login-pass"
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••••"
            />
          </div>
          <div className="login-options">
            <label className="remember" onClick={() => setRemember(!remember)}>
              <span className="checkbox" style={{ background: remember ? "var(--primary)" : "var(--bg-input)", border: remember ? "none" : "1px solid var(--border)" }}>
                {remember && (
                  <svg viewBox="0 0 10 10">
                    <polyline points="1,5.2 3.8,8 9,2" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                )}
              </span>
              记住我
            </label>
            <span className="login-forgot">忘记密码？</span>
          </div>
          {error && <p className="error" style={{ marginTop: 12 }}>{error}</p>}
          <button className="login-btn" type="submit" disabled={submitting}>
            {submitting ? "登录中..." : "登 录"}
          </button>
        </form>
        <div className="login-note">首次启动默认账号 admin / admin123，请登录后修改密码</div>
      </div>
      <div className="login-copyright">Copyright 2026 Component Library MCP Server</div>
      <div className="login-version">v0.1.0</div>
    </div>
  );
}
