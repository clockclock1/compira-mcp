import { Navigate, Route, Routes } from "react-router-dom";
import { AuthProvider } from "./auth/AuthContext";
import AppLayout from "./components/AppLayout";
import ProtectedLayout from "./components/ProtectedLayout";
import Dashboard from "./pages/Dashboard";
import Libraries from "./pages/Libraries";
import LibraryDetail from "./pages/LibraryDetail";
import ComponentDetail from "./pages/ComponentDetail";
import McpPlayground from "./pages/McpPlayground";
import McpLive from "./pages/McpLive";
import ApiKeys from "./pages/ApiKeys";
import Users from "./pages/Users";
import Logs from "./pages/Logs";
import Login from "./pages/Login";
import Settings from "./pages/Settings";

export default function App() {
  return (
    <AuthProvider>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route element={<ProtectedLayout />}>
          <Route element={<AppLayout />}>
            <Route path="/" element={<Dashboard />} />
            <Route path="/libraries" element={<Libraries />} />
            <Route path="/libraries/:id" element={<LibraryDetail />} />
            <Route path="/components/:id" element={<ComponentDetail />} />
            <Route path="/mcp" element={<McpPlayground />} />
            <Route path="/mcp/live" element={<McpLive />} />
            <Route path="/api-keys" element={<ApiKeys />} />
            <Route path="/users" element={<Users />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/logs" element={<Logs />} />
          </Route>
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </AuthProvider>
  );
}
