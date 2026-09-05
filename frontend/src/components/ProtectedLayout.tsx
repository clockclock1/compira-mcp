import { Navigate, Outlet } from "react-router-dom";
import { useAuth } from "../auth/AuthContext";

export default function ProtectedLayout() {
  const { user, loading } = useAuth();

  if (loading) {
    return <div className="loading" style={{ minHeight: "100vh" }}>加载中...</div>;
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  return <Outlet />;
}
