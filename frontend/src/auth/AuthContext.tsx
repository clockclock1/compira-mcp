import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  ReactNode,
} from "react";
import {
  api,
  User,
  clearToken,
  getExpiresAt,
  getToken,
  setExpiresAt,
  setToken,
} from "../api/client";

interface AuthContextValue {
  user: User | null;
  loading: boolean;
  expiresAt: string | null;
  login: (username: string, password: string, rememberMe?: boolean) => Promise<void>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
  isAdmin: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [expiresAt, setExpiresAtState] = useState<string | null>(getExpiresAt());
  const [loading, setLoading] = useState(true);
  const expiryTimer = useRef<number | null>(null);

  const clearExpiryTimer = () => {
    if (expiryTimer.current != null) {
      window.clearTimeout(expiryTimer.current);
      expiryTimer.current = null;
    }
  };

  const scheduleExpiry = useCallback((iso: string | null) => {
    clearExpiryTimer();
    if (!iso) return;
    const ms = new Date(iso).getTime() - Date.now();
    if (Number.isNaN(ms)) return;
    if (ms <= 0) {
      clearToken();
      setUser(null);
      setExpiresAtState(null);
      return;
    }
    // Cap setTimeout to ~24h chunks to avoid overflow; re-check later.
    const wait = Math.min(ms, 24 * 60 * 60 * 1000);
    expiryTimer.current = window.setTimeout(() => {
      const left = new Date(iso).getTime() - Date.now();
      if (left <= 0) {
        clearToken();
        setUser(null);
        setExpiresAtState(null);
        window.location.href = "/login";
      } else {
        scheduleExpiry(iso);
      }
    }, wait);
  }, []);

  const refresh = useCallback(async () => {
    if (!getToken()) {
      setUser(null);
      setExpiresAtState(null);
      setLoading(false);
      return;
    }
    try {
      const res = await api.auth.me();
      setUser(res.user);
      const exp = res.expires_at || getExpiresAt();
      if (exp) {
        setExpiresAt(exp);
        setExpiresAtState(exp);
        scheduleExpiry(exp);
      }
    } catch {
      clearToken();
      setUser(null);
      setExpiresAtState(null);
    } finally {
      setLoading(false);
    }
  }, [scheduleExpiry]);

  useEffect(() => {
    refresh();
    return () => clearExpiryTimer();
  }, [refresh]);

  // Keep client expiry in sync when sliding sessions extend on the server.
  useEffect(() => {
    const quiet = () => {
      if (getToken()) void refresh();
    };
    const onFocus = () => quiet();
    window.addEventListener("focus", onFocus);
    const id = window.setInterval(quiet, 30 * 60 * 1000);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.clearInterval(id);
    };
  }, [refresh]);

  const login = async (username: string, password: string, rememberMe = true) => {
    const res = await api.auth.login(username, password, rememberMe);
    setToken(res.token, { remember: res.remember_me, expiresAt: res.expires_at });
    setUser(res.user);
    setExpiresAtState(res.expires_at);
    scheduleExpiry(res.expires_at);
  };

  const logout = async () => {
    try {
      await api.auth.logout();
    } catch {
      /* ignore */
    }
    clearExpiryTimer();
    clearToken();
    setUser(null);
    setExpiresAtState(null);
  };

  const refreshUser = async () => {
    await refresh();
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        loading,
        expiresAt,
        login,
        logout,
        refreshUser,
        isAdmin: user?.role === "admin",
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
