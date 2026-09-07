import type {
  Component,
  ComponentExample,
  ComponentSearchResult,
  McpTool,
  User,
} from "./types";

export * from "./types";

const TOKEN_STORAGE = "compira_token";
const EXPIRES_STORAGE = "compira_expires_at";
const REMEMBER_STORAGE = "compira_remember";

function tokenStore(remember?: boolean): Storage {
  if (remember === undefined) {
    const flag = localStorage.getItem(REMEMBER_STORAGE);
    if (flag === "0") return sessionStorage;
    return localStorage;
  }
  return remember ? localStorage : sessionStorage;
}

export function getToken(): string {
  return (
    localStorage.getItem(TOKEN_STORAGE) ||
    sessionStorage.getItem(TOKEN_STORAGE) ||
    ""
  );
}

export function getExpiresAt(): string | null {
  return (
    localStorage.getItem(EXPIRES_STORAGE) ||
    sessionStorage.getItem(EXPIRES_STORAGE) ||
    null
  );
}

export function setToken(token: string, opts?: { remember?: boolean; expiresAt?: string }) {
  const remember = opts?.remember ?? true;
  clearToken();
  localStorage.setItem(REMEMBER_STORAGE, remember ? "1" : "0");
  const store = tokenStore(remember);
  store.setItem(TOKEN_STORAGE, token);
  if (opts?.expiresAt) {
    store.setItem(EXPIRES_STORAGE, opts.expiresAt);
  }
}

export function setExpiresAt(expiresAt: string) {
  const remember = localStorage.getItem(REMEMBER_STORAGE) !== "0";
  tokenStore(remember).setItem(EXPIRES_STORAGE, expiresAt);
}

export function clearToken() {
  localStorage.removeItem(TOKEN_STORAGE);
  localStorage.removeItem(EXPIRES_STORAGE);
  sessionStorage.removeItem(TOKEN_STORAGE);
  sessionStorage.removeItem(EXPIRES_STORAGE);
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Content-Type", "application/json");
  const token = getToken();
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  const res = await fetch(`/api${path}`, { ...options, headers });
  if (res.status === 401 && path !== "/auth/login") {
    clearToken();
    window.location.href = "/login";
    throw new Error("未登录或会话已过期");
  }
  if (!res.ok) {
    const body = await res.text();
    if (res.status === 403 && path === "/auth/account") {
      throw new Error(body || "当前密码不正确");
    }
    if (res.status === 409 && path === "/auth/account") {
      throw new Error(body || "用户名已被占用");
    }
    throw new Error(body || res.statusText);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

export interface Library {
  id: string;
  name: string;
  repo_url: string;
  branch: string;
  local_path: string;
  status: string;
  component_count: number;
  rules: string | null;
  source_type?: string;
  last_error?: string | null;
  last_synced_at: string | null;
  created_at: string;
}

export interface SyncTask {
  id: string;
  library_id: string;
  status: string;
  progress: number;
  message: string;
  started_at: string;
  finished_at: string | null;
}

export interface ApiKey {
  id: string;
  name: string;
  key_prefix: string;
  /** Full secret when available (admin UI). */
  key?: string | null;
  created_at: string;
  last_used_at: string | null;
}

export interface LogEntry {
  id: number;
  level: string;
  message: string;
  context: string | null;
  created_at: string;
}

export interface Stats {
  libraries: number;
  components: number;
  api_keys: number;
  users: number;
}

export const api = {
  auth: {
    login: (username: string, password: string, rememberMe = true) =>
      request<{
        token: string;
        user: User;
        expires_at: string;
        remember_me: boolean;
      }>("/auth/login", {
        method: "POST",
        body: JSON.stringify({
          username,
          password,
          remember_me: rememberMe,
        }),
      }),
    logout: () => request<void>("/auth/logout", { method: "POST" }),
    me: () =>
      request<{ user: User; expires_at?: string | null }>("/auth/me"),
    updateAccount: (data: {
      username?: string;
      current_password: string;
      new_password?: string;
    }) =>
      request<{ user: User; expires_at?: string | null }>("/auth/account", {
        method: "PATCH",
        body: JSON.stringify(data),
      }),
  },
  users: {
    list: () => request<User[]>("/users"),
    create: (data: {
      username: string;
      password: string;
      role?: string;
      display_name?: string;
    }) => request<User>("/users", { method: "POST", body: JSON.stringify(data) }),
    update: (id: string, data: { password?: string; role?: string }) =>
      request<User>(`/users/${id}`, { method: "PATCH", body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/users/${id}`, { method: "DELETE" }),
  },
  stats: () => request<Stats>("/stats"),
  libraries: {
    list: () => request<Library[]>("/libraries"),
    get: (id: string) => request<Library>(`/libraries/${id}`),
    components: (id: string, opts?: { limit?: number; offset?: number }) => {
      const limit = opts?.limit ?? 500;
      const offset = opts?.offset ?? 0;
      return request<{
        items: Component[];
        total: number;
        limit: number;
        offset: number;
      }>(`/libraries/${id}/components?limit=${limit}&offset=${offset}`);
    },
    create: (data: {
      name: string;
      repo_url?: string;
      branch?: string;
      rules?: string;
      source_type?: "git" | "upload" | "fetch";
    }) => request<Library>("/libraries", { method: "POST", body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/libraries/${id}`, { method: "DELETE" }),
    sync: (id: string) =>
      request<{ task_id: string }>(`/libraries/${id}/sync`, { method: "POST" }),
    upload: async (id: string, files: File[], useAi = true, autoName = false) => {
      const form = new FormData();
      form.append("use_ai", useAi ? "true" : "false");
      form.append("auto_name", autoName ? "true" : "false");
      for (const f of files) form.append("files", f);
      const headers = new Headers();
      const token = getToken();
      if (token) headers.set("Authorization", `Bearer ${token}`);
      const res = await fetch(`/api/libraries/${id}/upload`, {
        method: "POST",
        headers,
        body: form,
      });
      if (!res.ok) throw new Error((await res.text()) || res.statusText);
      return res.json() as Promise<{
        task_id: string;
        files: string[];
        use_ai: boolean;
        auto_name: boolean;
      }>;
    },
    fetch: (id: string, data: { prompt: string; auto_name?: boolean }) =>
      request<{ task_id: string; auto_name?: boolean }>(`/libraries/${id}/fetch`, {
        method: "POST",
        body: JSON.stringify(data),
      }),
  },
  ai: {
    status: () =>
      request<{
        enabled: boolean;
        model: string;
        base_url: string;
        api_key_set?: boolean;
        api_key_masked?: string | null;
      }>("/ai/status"),
  },
  settings: {
    llm: () =>
      request<{
        enabled: boolean;
        api_key_set: boolean;
        api_key_masked: string | null;
        base_url: string;
        model: string;
      }>("/settings/llm"),
    updateLlm: (data: {
      api_key?: string;
      base_url?: string;
      model?: string;
      clear_api_key?: boolean;
    }) =>
      request<{
        enabled: boolean;
        api_key_set: boolean;
        api_key_masked: string | null;
        base_url: string;
        model: string;
      }>("/settings/llm", { method: "PUT", body: JSON.stringify(data) }),
    sync: () =>
      request<{
        max_jobs: number;
        parse_concurrency: number;
        ingest_batch_size: number;
        download_concurrency: number;
        fetch_max_attempts: number;
      }>("/settings/sync"),
    updateSync: (data: {
      max_jobs?: number;
      parse_concurrency?: number;
      ingest_batch_size?: number;
      download_concurrency?: number;
      fetch_max_attempts?: number;
    }) =>
      request<{
        max_jobs: number;
        parse_concurrency: number;
        ingest_batch_size: number;
        download_concurrency: number;
        fetch_max_attempts: number;
      }>("/settings/sync", { method: "PUT", body: JSON.stringify(data) }),
    auth: () =>
      request<{
        session_ttl_hours: number;
        remember_me_ttl_hours: number;
        sliding: boolean;
      }>("/settings/auth"),
    updateAuth: (data: {
      session_ttl_hours?: number;
      remember_me_ttl_hours?: number;
      sliding?: boolean;
    }) =>
      request<{
        session_ttl_hours: number;
        remember_me_ttl_hours: number;
        sliding: boolean;
      }>("/settings/auth", { method: "PUT", body: JSON.stringify(data) }),
  },
  admin: {
    storageStats: () =>
      request<{
        data_dir: string;
        repos_dir: string;
        total_bytes: number;
        database: {
          path: string;
          file_bytes: number;
          wal_bytes: number;
          shm_bytes: number;
          page_bytes: number;
          components_source_bytes: number;
          components_rows: number;
          libraries_rows: number;
          sync_tasks_rows: number;
          app_logs_rows: number;
          sessions_rows: number;
        };
        repos: {
          total_bytes: number;
          library_dirs: number;
          orphan_dirs: number;
          entries: {
            id: string;
            name: string | null;
            bytes: number;
            source_type: string | null;
            orphan: boolean;
          }[];
        };
        reclaimable_bytes_estimate: number;
      }>("/admin/storage/stats"),
    cleanup: (data?: {
      dry_run?: boolean;
      orphan_repos?: boolean;
      sync_tasks_older_than_days?: number;
      app_logs_older_than_days?: number;
      expired_sessions?: boolean;
      wal_checkpoint?: boolean;
      vacuum?: boolean;
    }) =>
      request<{
        dry_run: boolean;
        orphan_repos_removed: number;
        orphan_repos_bytes: number;
        sync_tasks_deleted: number;
        app_logs_deleted: number;
        sessions_deleted: number;
        wal_checkpoint: boolean;
        vacuum: boolean;
        bytes_freed_estimate: number;
        messages: string[];
        errors: string[];
      }>("/admin/storage/cleanup", {
        method: "POST",
        body: JSON.stringify(data || {}),
      }),
    cleanupSchedule: () =>
      request<{
        enabled: boolean;
        interval_hours: number;
        orphan_repos: boolean;
        sync_tasks_older_than_days: number;
        app_logs_older_than_days: number;
        expired_sessions: boolean;
        wal_checkpoint: boolean;
        vacuum: boolean;
        last_run_at: string | null;
        last_result: string | null;
      }>("/admin/storage/schedule"),
    updateCleanupSchedule: (data: {
      enabled: boolean;
      interval_hours: number;
      orphan_repos: boolean;
      sync_tasks_older_than_days: number;
      app_logs_older_than_days: number;
      expired_sessions: boolean;
      wal_checkpoint: boolean;
      vacuum: boolean;
      last_run_at?: string | null;
      last_result?: string | null;
    }) =>
      request<{
        enabled: boolean;
        interval_hours: number;
        orphan_repos: boolean;
        sync_tasks_older_than_days: number;
        app_logs_older_than_days: number;
        expired_sessions: boolean;
        wal_checkpoint: boolean;
        vacuum: boolean;
        last_run_at: string | null;
        last_result: string | null;
      }>("/admin/storage/schedule", { method: "PUT", body: JSON.stringify(data) }),
    memory: () =>
      request<{
        process: {
          pid: number;
          working_set_bytes: number;
          virtual_bytes: number;
        };
        task_manager: { max_jobs: number; active_jobs: number };
        sqlite: {
          cache_size_kib: number;
          mmap_size: number;
          temp_store: string;
        };
        breakdown: {
          key: string;
          label: string;
          bytes: number | null;
          detail: string;
        }[];
        notes: string[];
      }>("/admin/memory"),
  },
  tasks: {
    list: (libraryId?: string) =>
      request<SyncTask[]>(`/tasks${libraryId ? `?library_id=${libraryId}` : ""}`),
    get: (id: string) => request<SyncTask>(`/tasks/${id}`),
  },
  apiKeys: {
    list: () => request<ApiKey[]>("/api-keys"),
    create: (name: string) =>
      request<{ key: ApiKey }>("/api-keys", {
        method: "POST",
        body: JSON.stringify({ name }),
      }),
    regenerate: (id: string) =>
      request<{ key: ApiKey }>(`/api-keys/${id}/regenerate`, { method: "POST" }),
    delete: (id: string) => request<void>(`/api-keys/${id}`, { method: "DELETE" }),
  },
  logs: (limit = 100) => request<LogEntry[]>(`/logs?limit=${limit}`),
  search: {
    components: (q: string, libraryId?: string, limit = 20) =>
      request<ComponentSearchResult[]>(
        `/search/components?q=${encodeURIComponent(q)}${libraryId ? `&library_id=${libraryId}` : ""}&limit=${limit}`,
      ),
  },
  components: {
    get: (id: string) => request<Component>(`/components/${id}`),
    source: (id: string) => request<{ source: string }>(`/components/${id}/source`),
    docs: (id: string) => request<{ docs: string }>(`/components/${id}/docs`),
    examples: (id: string) => request<ComponentExample[]>(`/components/${id}/examples`),
  },
  mcp: {
    tools: () => request<McpTool[]>("/mcp/tools"),
    call: (tool: string, arguments_: object) =>
      request<{ ok: boolean; result?: unknown; error?: string }>("/mcp/call", {
        method: "POST",
        body: JSON.stringify({ tool, arguments: arguments_ }),
      }),
    activity: (limit = 100) =>
      request<
        {
          id: string;
          started_at: string;
          finished_at: string | null;
          tool: string;
          args_summary: string;
          duration_ms: number | null;
          ok: boolean | null;
          error: string | null;
          source: string;
          api_key_id: string | null;
          api_key_name: string | null;
          user_id: string | null;
          username: string | null;
          status: string;
        }[]
      >(`/mcp/activity?limit=${limit}`),
    clearActivity: () => request<void>("/mcp/activity", { method: "DELETE" }),
  },
};
