import type {
  Component,
  ComponentExample,
  ComponentSearchResult,
  McpTool,
  User,
} from "./types";

export * from "./types";

const TOKEN_STORAGE = "compira_token";

export function getToken(): string {
  return localStorage.getItem(TOKEN_STORAGE) || "";
}

export function setToken(token: string) {
  localStorage.setItem(TOKEN_STORAGE, token);
}

export function clearToken() {
  localStorage.removeItem(TOKEN_STORAGE);
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
    login: (username: string, password: string) =>
      request<{ token: string; user: User }>("/auth/login", {
        method: "POST",
        body: JSON.stringify({ username, password }),
      }),
    logout: () => request<void>("/auth/logout", { method: "POST" }),
    me: () => request<{ user: User }>("/auth/me"),
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
    components: (id: string) => request<Component[]>(`/libraries/${id}/components`),
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
  },
  tasks: {
    list: (libraryId?: string) =>
      request<SyncTask[]>(`/tasks${libraryId ? `?library_id=${libraryId}` : ""}`),
    get: (id: string) => request<SyncTask>(`/tasks/${id}`),
  },
  apiKeys: {
    list: () => request<ApiKey[]>("/api-keys"),
    create: (name: string) =>
      request<{ key: { id: string; name: string; key: string; key_prefix: string } }>(
        "/api-keys",
        { method: "POST", body: JSON.stringify({ name }) },
      ),
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
  },
};
