import { ReactNode } from "react";
import { NavLink, Outlet } from "react-router-dom";
import { useAuth } from "../auth/AuthContext";

function LogoIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 17l6-6-6-6" />
      <path d="M12 19h8" />
    </svg>
  );
}

const navItems: { to: string; label: string; adminOnly?: boolean; icon: ReactNode; end?: boolean }[] = [
  {
    to: "/",
    end: true,
    label: "仪表盘",
    icon: (
      <svg viewBox="0 0 24 24">
        <rect x="3" y="3" width="7" height="9" rx="1.5" />
        <rect x="14" y="3" width="7" height="5" rx="1.5" />
        <rect x="14" y="12" width="7" height="9" rx="1.5" />
        <rect x="3" y="16" width="7" height="5" rx="1.5" />
      </svg>
    ),
  },
  {
    to: "/libraries",
    label: "组件库",
    icon: (
      <svg viewBox="0 0 24 24">
        <path d="M3 7l9-4 9 4-9 4-9-4z" />
        <path d="M3 7v10l9 4 9-4V7" />
        <path d="M12 11v10" />
      </svg>
    ),
  },
  {
    to: "/mcp",
    label: "MCP 调试",
    icon: (
      <svg viewBox="0 0 24 24">
        <rect x="2" y="4" width="20" height="16" rx="2" />
        <path d="M7 15h10" />
        <path d="M7 9h10" />
      </svg>
    ),
  },
  {
    to: "/api-keys",
    label: "API Keys",
    adminOnly: true,
    icon: (
      <svg viewBox="0 0 24 24">
        <circle cx="8" cy="15" r="4" />
        <path d="M10.8 12.2L21 2" />
        <path d="M17 6l3 3" />
        <path d="M14 9l2 2" />
      </svg>
    ),
  },
  {
    to: "/users",
    label: "用户管理",
    adminOnly: true,
    icon: (
      <svg viewBox="0 0 24 24">
        <circle cx="9" cy="8" r="3.5" />
        <path d="M2.5 20c.8-3.5 3.5-5.5 6.5-5.5s5.7 2 6.5 5.5" />
        <circle cx="17.5" cy="9" r="2.5" />
        <path d="M16.5 14.8c2.6.3 4.4 2 5 5.2" />
      </svg>
    ),
  },
  {
    to: "/settings",
    label: "系统设置",
    adminOnly: true,
    icon: (
      <svg viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="3" />
        <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
      </svg>
    ),
  },
  {
    to: "/logs",
    label: "日志",
    icon: (
      <svg viewBox="0 0 24 24">
        <path d="M4 6h16" />
        <path d="M4 12h16" />
        <path d="M4 18h10" />
      </svg>
    ),
  },
];

export default function AppLayout() {
  const { user, logout, isAdmin } = useAuth();
  const displayName = user?.display_name || user?.username || "用户";
  const avatarChar = displayName.slice(0, 1);

  return (
    <div id="app" style={{ display: "block" }}>
      <aside className="sidebar">
        <div className="logo-row">
          <div className="logo-box">
            <LogoIcon />
          </div>
          <div className="logo-text">
            <div className="logo-title brand-display">CompiraMCP</div>
            <div className="logo-sub">COMPONENT LIBRARY</div>
          </div>
        </div>
        <nav className="nav">
          {navItems
            .filter((item) => !item.adminOnly || isAdmin)
            .map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
              >
                {item.icon}
                <span>{item.label}</span>
              </NavLink>
            ))}
        </nav>
        <div className="nav-spacer" />
        <div className="user-card">
          <div className="user-avatar">{avatarChar}</div>
          <div className="user-text">
            <div className="user-name">{displayName}</div>
            <div className="user-meta">
              {user?.username} · {user?.role === "admin" ? "管理员" : "用户"}
            </div>
          </div>
          <button type="button" className="btn-logout" onClick={() => logout()} title="退出登录">
            退出
          </button>
        </div>
      </aside>
      <main className="content">
        <Outlet />
      </main>
    </div>
  );
}
