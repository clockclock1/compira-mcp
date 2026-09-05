import { FormEvent, useEffect, useState } from "react";
import { api, User } from "../api/client";
import { useAuth } from "../auth/AuthContext";

export default function Users() {
  const { isAdmin } = useAuth();
  const [users, setUsers] = useState<User[]>([]);
  const [error, setError] = useState("");
  const [showModal, setShowModal] = useState(false);
  const [form, setForm] = useState({
    username: "",
    password: "",
    role: "user",
    display_name: "",
  });

  const load = () => {
    api.users
      .list()
      .then(setUsers)
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    if (isAdmin) load();
  }, [isAdmin]);

  if (!isAdmin) {
    return <div className="card empty">需要管理员权限</div>;
  }

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    try {
      await api.users.create({
        username: form.username,
        password: form.password,
        role: form.role,
        display_name: form.display_name || undefined,
      });
      setShowModal(false);
      setForm({ username: "", password: "", role: "user", display_name: "" });
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("确定删除此用户？")) return;
    try {
      await api.users.delete(id);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "删除失败");
    }
  };

  const handleResetPassword = async (id: string) => {
    const password = prompt("输入新密码（至少 6 位）");
    if (!password || password.length < 6) return;
    try {
      await api.users.update(id, { password });
      alert("密码已更新");
    } catch (err) {
      setError(err instanceof Error ? err.message : "更新失败");
    }
  };

  const toggleRole = async (user: User) => {
    const newRole = user.role === "admin" ? "user" : "admin";
    try {
      await api.users.update(user.id, { role: newRole });
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "更新失败");
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>用户管理</h1>
          <div className="page-sub">管理后台账号：增删改、重置密码、切换 admin / user 角色</div>
        </div>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          + 添加用户
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      <div className="card">
        <div className="card-header">
          <div className="card-title">用户列表</div>
          <span className="chip-count">{users.length}</span>
        </div>
        <div className="divider" />
        <table className="table">
          <thead>
            <tr>
              <th style={{ width: 220 }}>用户</th>
              <th style={{ width: 160 }}>显示名</th>
              <th style={{ width: 120 }}>角色</th>
              <th style={{ width: 160 }}>创建时间</th>
              <th style={{ width: 160 }}>最后登录</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {users.map((u) => {
              const avatar = (u.display_name || u.username).slice(0, 1);
              return (
                <tr key={u.id}>
                  <td>
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                      <div className="user-avatar" style={{ width: 28, height: 28, fontSize: 12 }}>
                        {avatar}
                      </div>
                      <span
                        style={{
                          fontSize: 12,
                          fontWeight: 500,
                          color: "var(--text-1)",
                          fontFamily: "var(--font-en)",
                        }}
                      >
                        {u.username}
                      </span>
                    </div>
                  </td>
                  <td style={{ fontSize: 12, color: "var(--text-2)" }}>{u.display_name || "—"}</td>
                  <td>
                    <span className="chip-role">{u.role}</span>
                  </td>
                  <td style={{ fontSize: 11, color: "var(--text-2)", fontFamily: "var(--font-mono)" }}>
                    {new Date(u.created_at).toLocaleDateString()}
                  </td>
                  <td style={{ fontSize: 11, color: "var(--text-2)" }}>
                    {u.last_login_at ? new Date(u.last_login_at).toLocaleString() : "—"}
                  </td>
                  <td>
                    <div style={{ display: "flex", gap: 16 }}>
                      <span className="link-action muted" onClick={() => handleResetPassword(u.id)}>
                        改密
                      </span>
                      <span className="link-action" onClick={() => toggleRole(u)}>
                        切换角色
                      </span>
                      <span className="link-action danger" onClick={() => handleDelete(u.id)}>
                        删除
                      </span>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className={`modal-backdrop${showModal ? " open" : ""}`} onClick={() => setShowModal(false)}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header">
            <div className="modal-title">添加用户</div>
            <div className="modal-sub">创建后台登录账号</div>
          </div>
          <form onSubmit={handleCreate}>
            <div className="modal-body">
              <div className="modal-field">
                <label>用户名</label>
                <input
                  className="modal-input"
                  required
                  minLength={3}
                  value={form.username}
                  onChange={(e) => setForm({ ...form, username: e.target.value })}
                />
              </div>
              <div className="modal-field">
                <label>密码</label>
                <input
                  className="modal-input"
                  required
                  type="password"
                  minLength={6}
                  value={form.password}
                  onChange={(e) => setForm({ ...form, password: e.target.value })}
                />
              </div>
              <div className="modal-field">
                <label>显示名</label>
                <input
                  className="modal-input"
                  value={form.display_name}
                  onChange={(e) => setForm({ ...form, display_name: e.target.value })}
                />
              </div>
              <div className="modal-field">
                <label>角色</label>
                <select
                  className="modal-select"
                  value={form.role}
                  onChange={(e) => setForm({ ...form, role: e.target.value })}
                >
                  <option value="user">user</option>
                  <option value="admin">admin</option>
                </select>
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
