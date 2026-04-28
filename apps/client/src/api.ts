import type {
  Session,
  ChatMessage,
  Project,
  AutomationTask,
  AppConfig,
  PluginSettings,
} from "./types";

// ---------------------------------------------------------------------------
// Tauri invoke wrapper — falls back to mock data when running in browser
// ---------------------------------------------------------------------------

const isTauri = (() => {
  try {
    return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
  } catch {
    return false;
  }
})();

// Browser remote API base (used when not in Tauri)
function inferBrowserApiBase(): string {
  if (typeof window === "undefined") return "http://127.0.0.1:8787";
  const protocol = window.location.protocol === "https:" ? "https:" : "http:";
  const host = window.location.hostname || "127.0.0.1";
  return `${protocol}//${host}:8787`;
}

function normalizeBrowserApiBase(raw: string): string {
  const trimmed = normalizeApiBase(raw.trim());
  // Migrate legacy local default to cloud/dev server default.
  if (trimmed === "http://127.0.0.1:3000" || trimmed === "http://localhost:3000") {
    return inferBrowserApiBase();
  }
  return trimmed;
}

let browserApiBase = inferBrowserApiBase();
const LS_TOKEN_KEY = "hermes_api_token";
const PLUGIN_MIGRATION_FLAG = "hermes.plugins.remote.migrated.v1";
const LEGACY_CONFIG_KEY = "hermes.desktop.config";
const SESSIONS_STORAGE_KEY = "hermes.browser.sessions.v1";
const PROJECTS_STORAGE_KEY = "hermes.browser.projects.v1";
const CONFIG_STORAGE_KEY = "hermes.browser.config.v1";
const DEFAULT_PLUGIN_SETTINGS: PluginSettings = {
  mcp_filesystem: true,
  mcp_terminal: true,
  mcp_browser: false,
  mcp_database: false,
  tool_code_exec: true,
};

function normalizeApiBase(raw: string): string {
  return raw.replace(/\/$/, "");
}

function getApiBase(): string {
  const fromEnv =
    typeof import.meta !== "undefined" && import.meta.env?.VITE_API_BASE_URL
      ? String(import.meta.env.VITE_API_BASE_URL)
      : "";
  const base = fromEnv || browserApiBase;
  return normalizeApiBase(base);
}

export function apiUrl(path: string): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  return `${getApiBase()}${p}`;
}

export function resolveWebSocketUrl(path: string, token?: string): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  const base = getApiBase();
  const origin = new URL(base);
  const wsProto = origin.protocol === "https:" ? "wss:" : "ws:";
  const ws = new URL(`${wsProto}//${origin.host}${p}`);
  const bearer = token ?? resolveBearerToken();
  if (bearer) ws.searchParams.set("token", bearer);
  return ws.toString();
}

export function resolveBearerToken(): string | null {
  try {
    const v = localStorage.getItem(LS_TOKEN_KEY);
    return v && v.trim() ? v.trim() : null;
  } catch {
    return null;
  }
}

async function fetchJSON<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  const token = resolveBearerToken();
  if (token && !headers.has("Authorization")) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  const response = await fetch(apiUrl(path), { ...init, headers });
  if (!response.ok) {
    throw new Error(await response.text().catch(() => response.statusText));
  }
  return response.json();
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
    return tauriInvoke<T>(cmd, args);
  }
  // Browser mode: use hermes-server API for messages, mock for UI state
  if (cmd === "send_message") {
    return browserSendMessage(args?.sessionId as string, args?.content as string) as T;
  }
  if (cmd === "check_backend_health") {
    return browserHealthCheck() as T;
  }
  return mockInvoke<T>(cmd, args);
}

async function browserHealthCheck(): Promise<unknown> {
  try {
    const resp = await fetch(`${browserApiBase}/health`, { signal: AbortSignal.timeout(3000) });
    const data = await resp.json();
    return data.status === "ok";
  } catch {
    return false;
  }
}

async function browserSendMessage(sessionId: string, content: string): Promise<unknown> {
  const now = new Date().toISOString();
  const userMsg: ChatMessage = { id: `b-${Date.now()}`, role: "user", content, timestamp: now };
  const session = mockSessions.find(s => s.id === sessionId);
  if (session) {
    session.messages.push(userMsg);
    if (session.messages.length === 1) session.title = content.slice(0, 30);
    session.updated_at = now;
    persistBrowserState();
  }

  // Connect WebSocket and start streaming (don't await completion)
  const wsBase = browserApiBase.replace("http://", "ws://").replace("https://", "wss://");
  const wsUrl = `${wsBase}/v1/ws-stream/${sessionId}`;

  try {
    const ws = new WebSocket(wsUrl);
    let fullReply = "";

    ws.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data);
        if (data.type === "connected") {
          ws.send(JSON.stringify({ text: content, user_id: "browser" }));
          return;
        }
        if (data.type === "text") {
          fullReply += data.content;
        }
        // Dispatch streaming event for UI
        window.dispatchEvent(new CustomEvent("hermes-stream", { detail: data }));

        if (data.type === "done" || data.type === "error") {
          // Add final assistant message to mock store
          const reply = data.type === "done" ? (data.content || fullReply) : `⚠️ ${data.content}`;
          const assistantMsg: ChatMessage = {
            id: `b-${Date.now()}-a`, role: "assistant", content: reply,
            timestamp: new Date().toISOString(), model: "remote",
          };
          if (session) {
            session.messages.push(assistantMsg);
            session.updated_at = new Date().toISOString();
            persistBrowserState();
          }
          ws.close();
        }
      } catch { /* ignore */ }
    };

    ws.onerror = () => {
      // Fall back to HTTP POST
      fetch(`${browserApiBase}/v1/sessions/${sessionId}/messages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text: content, user_id: "browser" }),
      })
        .then(r => r.json())
        .then(data => {
          const assistantMsg: ChatMessage = {
            id: `b-${Date.now()}-a`, role: "assistant", content: data.reply,
            timestamp: new Date().toISOString(), model: "remote",
          };
          if (session) {
            session.messages.push(assistantMsg);
            session.updated_at = new Date().toISOString();
            persistBrowserState();
          }
          window.dispatchEvent(new CustomEvent("hermes-stream", { detail: { type: "done", content: data.reply } }));
        })
        .catch(e => {
          window.dispatchEvent(new CustomEvent("hermes-stream", { detail: { type: "done", content: `⚠️ 连接失败: ${e}` } }));
        });
    };
  } catch {
    // WebSocket not available
  }

  // Return immediately — streaming events will drive the UI
  return userMsg;
}

// ---------------------------------------------------------------------------
// In-memory mock store for browser development
// ---------------------------------------------------------------------------

let mockSessions: Session[] = [];
let mockProjects: Project[] = [
  { id: "p1", name: "hermes-agent-rust", path: "/Users/ly/workspace/research/hermes-agent-rust" },
];
let mockConfig: AppConfig = {
  api_base: inferBrowserApiBase(),
  default_model: "",
  theme: "dark",
  mode: "remote",  // Browser always uses remote mode
};

let mockPluginSettings: PluginSettings = { ...DEFAULT_PLUGIN_SETTINGS };

function loadBrowserState() {
  if (isTauri || typeof localStorage === "undefined") return;
  try {
    const rawSessions = localStorage.getItem(SESSIONS_STORAGE_KEY);
    const rawProjects = localStorage.getItem(PROJECTS_STORAGE_KEY);
    const rawConfig = localStorage.getItem(CONFIG_STORAGE_KEY);
    if (rawSessions) {
      const parsed = JSON.parse(rawSessions);
      if (Array.isArray(parsed)) mockSessions = parsed;
    }
    if (rawProjects) {
      const parsed = JSON.parse(rawProjects);
      if (Array.isArray(parsed)) mockProjects = parsed;
    }
    if (rawConfig) {
      const parsed = JSON.parse(rawConfig);
      if (parsed && typeof parsed === "object") {
        mockConfig = { ...mockConfig, ...parsed };
        if (typeof mockConfig.api_base === "string" && mockConfig.api_base.trim()) {
          const normalized = normalizeBrowserApiBase(mockConfig.api_base);
          browserApiBase = normalized;
          mockConfig.api_base = normalized;
        }
      }
    }
  } catch {
    // ignore corrupted browser state
  }
}

function persistBrowserState() {
  if (isTauri || typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(SESSIONS_STORAGE_KEY, JSON.stringify(mockSessions));
    localStorage.setItem(PROJECTS_STORAGE_KEY, JSON.stringify(mockProjects));
    localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(mockConfig));
  } catch {
    // best-effort persistence
  }
}

loadBrowserState();

let idCounter = 1;
function uid() {
  return `mock-${Date.now()}-${idCounter++}`;
}

async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const now = new Date().toISOString();
  switch (cmd) {
    case "get_sessions": return mockSessions as T;
    case "create_session": {
      const s: Session = { id: uid(), title: (args?.title as string) || "New chat", project: args?.project as string | undefined, messages: [], created_at: now, updated_at: now };
      mockSessions = [s, ...mockSessions];
      persistBrowserState();
      return s as T;
    }
    case "delete_session": {
      mockSessions = mockSessions.filter(s => s.id !== args?.sessionId);
      persistBrowserState();
      return true as T;
    }
    case "rename_session": {
      const s = mockSessions.find(s => s.id === args?.sessionId);
      if (s) { s.title = args?.title as string; s.updated_at = now; }
      persistBrowserState();
      return !!s as T;
    }
    case "get_session_messages": return (mockSessions.find(s => s.id === args?.sessionId)?.messages ?? []) as T;
    case "send_message":
    case "send_message_stream": {
      const sid = args?.sessionId as string;
      const content = args?.content as string;
      const session = mockSessions.find(s => s.id === sid);
      if (session) {
        const userMsg: ChatMessage = { id: uid(), role: "user", content, timestamp: now };
        const assistantMsg: ChatMessage = { id: uid(), role: "assistant", content: generateMockResponse(content), timestamp: new Date().toISOString(), model: mockConfig.default_model };
        session.messages.push(userMsg, assistantMsg);
        session.updated_at = new Date().toISOString();
        if (session.messages.length === 2) session.title = content.slice(0, 30) + (content.length > 30 ? "..." : "");
        persistBrowserState();
        return userMsg as T;
      }
      return null as T;
    }
    case "get_projects": return mockProjects as T;
    case "add_project": {
      const p: Project = { id: uid(), name: args?.name as string, path: args?.path as string };
      mockProjects.push(p);
      persistBrowserState();
      return p as T;
    }
    case "remove_project": {
      mockProjects = mockProjects.filter(p => p.id !== args?.projectId);
      persistBrowserState();
      return true as T;
    }
    case "get_config": return mockConfig as T;
    case "update_config": {
      mockConfig = args?.config as AppConfig;
      if (typeof mockConfig.api_base === "string" && mockConfig.api_base.trim()) {
        const normalized = normalizeBrowserApiBase(mockConfig.api_base);
        browserApiBase = normalized;
        mockConfig.api_base = normalized;
      }
      persistBrowserState();
      return mockConfig as T;
    }
    case "check_backend_health": return false as T;
    case "exec_command": return "Mock: command executed" as T;
    case "get_automation_tasks": return [
      { id: "1", title: "Summarize yesterday's git activity for standup.", description: "Status reports", icon: "📝", category: "Status reports" },
      { id: "2", title: "Synthesize this week's PRs, rollouts, incidents, and reviews into a weekly update.", description: "Status reports", icon: "📋", category: "Status reports" },
      { id: "3", title: "Summarize last week's PRs by teammate and theme; highlight risks.", description: "Status reports", icon: "🖥️", category: "Status reports" },
      { id: "4", title: "Draft weekly release notes from merged PRs (include links when available).", description: "Release prep", icon: "📮", category: "Release prep" },
      { id: "5", title: "Before tagging, verify changelog, migrations, feature flags, and tests.", description: "Release prep", icon: "✅", category: "Release prep" },
      { id: "6", title: "Update the changelog with this week's highlights and key PR links.", description: "Release prep", icon: "✏️", category: "Release prep" },
      { id: "7", title: "Triage new issues: label, assign, and flag anything blocking.", description: "Incidents & triage", icon: "🔍", category: "Incidents & triage" },
      { id: "8", title: "Summarize open incidents and their current status.", description: "Incidents & triage", icon: "💬", category: "Incidents & triage" },
    ] as T;
    default: return null as T;
  }
}

type RemotePluginPayload = {
  plugins?: PluginSettings;
  persisted?: boolean;
};

function normalizePluginSettings(input?: Partial<PluginSettings>): PluginSettings {
  return {
    mcp_filesystem: input?.mcp_filesystem ?? DEFAULT_PLUGIN_SETTINGS.mcp_filesystem,
    mcp_terminal: input?.mcp_terminal ?? DEFAULT_PLUGIN_SETTINGS.mcp_terminal,
    mcp_browser: input?.mcp_browser ?? DEFAULT_PLUGIN_SETTINGS.mcp_browser,
    mcp_database: input?.mcp_database ?? DEFAULT_PLUGIN_SETTINGS.mcp_database,
    tool_code_exec: input?.tool_code_exec ?? DEFAULT_PLUGIN_SETTINGS.tool_code_exec,
  };
}

function loadLegacyPluginSettings(): PluginSettings | null {
  try {
    const raw = localStorage.getItem(LEGACY_CONFIG_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { plugins?: Partial<PluginSettings> } & Partial<PluginSettings>;
    if (parsed.plugins) return normalizePluginSettings(parsed.plugins);
    if (
      parsed.mcp_filesystem !== undefined ||
      parsed.mcp_terminal !== undefined ||
      parsed.mcp_browser !== undefined ||
      parsed.mcp_database !== undefined ||
      parsed.tool_code_exec !== undefined
    ) {
      return normalizePluginSettings(parsed);
    }
    return null;
  } catch {
    return null;
  }
}

async function migrateLegacyPluginSettingsOnce(
  apiBase: string,
  persisted: boolean,
): Promise<PluginSettings | null> {
  if (persisted) return null;
  try {
    if (localStorage.getItem(PLUGIN_MIGRATION_FLAG) === "done") return null;
    const legacy = loadLegacyPluginSettings();
    if (legacy) {
      await fetch(`${apiBase}/api/dashboard/plugins`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ plugins: legacy }),
      });
      localStorage.setItem(PLUGIN_MIGRATION_FLAG, "done");
      return legacy;
    }
    localStorage.setItem(PLUGIN_MIGRATION_FLAG, "done");
  } catch {
    // best-effort migration
  }
  return null;
}

async function resolveApiBaseForPluginSettings(): Promise<string> {
  if (!isTauri) return browserApiBase;
  const cfg = await invoke<AppConfig>("get_config");
  return cfg.api_base || browserApiBase;
}

export async function getPluginSettings(): Promise<PluginSettings> {
  const apiBase = await resolveApiBaseForPluginSettings();
  try {
    const response = await fetch(`${apiBase}/api/dashboard/plugins`);
    if (!response.ok) return { ...DEFAULT_PLUGIN_SETTINGS };
    const payload = (await response.json()) as RemotePluginPayload;
    const migrated = await migrateLegacyPluginSettingsOnce(apiBase, Boolean(payload.persisted));
    if (migrated) return normalizePluginSettings(migrated);
    if (payload.plugins) return normalizePluginSettings(payload.plugins);
  } catch {
    // fallback below
  }
  return { ...mockPluginSettings };
}

export async function updatePluginSettings(plugins: PluginSettings): Promise<PluginSettings> {
  const apiBase = await resolveApiBaseForPluginSettings();
  const normalized = normalizePluginSettings(plugins);
  try {
    const response = await fetch(`${apiBase}/api/dashboard/plugins`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ plugins: normalized }),
    });
    if (!response.ok) return normalized;
    const payload = (await response.json()) as RemotePluginPayload;
    const next = normalizePluginSettings(payload.plugins ?? normalized);
    mockPluginSettings = next;
    return next;
  } catch {
    mockPluginSettings = normalized;
    return normalized;
  }
}

function generateMockResponse(userMessage: string): string {
  return `收到你的消息: "${userMessage}"\n\n这是 **Mock 响应**。连接 Hermes 后端后将返回真实 AI 回复。\n\n\`\`\`rust\nlet response = agent.chat("${userMessage.slice(0, 20)}...").await?;\n\`\`\``;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export const getSessions = () => invoke<Session[]>("get_sessions");
export const createSession = (title: string, project?: string) => invoke<Session>("create_session", { title, project });
export const deleteSession = (sessionId: string) => invoke<boolean>("delete_session", { sessionId });
export const renameSession = (sessionId: string, title: string) => invoke<boolean>("rename_session", { sessionId, title });
export const getSessionMessages = (sessionId: string) => invoke<ChatMessage[]>("get_session_messages", { sessionId });
export const sendMessage = (sessionId: string, content: string) => invoke<ChatMessage>("send_message", { sessionId, content });
export const sendMessageStream = (sessionId: string, content: string) => invoke<ChatMessage>("send_message_stream", { sessionId, content });
export const getProjects = () => invoke<Project[]>("get_projects");
export const addProject = (name: string, path: string) => invoke<Project>("add_project", { name, path });
export const removeProject = (projectId: string) => invoke<boolean>("remove_project", { projectId });
export const getConfig = () => invoke<AppConfig>("get_config");
export const updateConfig = (config: AppConfig) => invoke<AppConfig>("update_config", { config });
export const getAutomationTasks = () => invoke<AutomationTask[]>("get_automation_tasks");
export const checkBackendHealth = () => invoke<boolean>("check_backend_health");
export const execCommand = (command: string, sessionId?: string) => invoke<string>("exec_command", { command, sessionId });

export interface AuthUserDto {
  id: string;
  email: string;
  tenant_id: string;
}

export interface AuthTokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
  user: AuthUserDto;
}

export interface CloudAgentGitPolicy {
  auto_commit_enabled: boolean;
  auto_push_enabled: boolean;
  target_branch: string;
  protected_branches: string[];
}

export interface CloudAgentSession {
  id: string;
  tenant_id: string;
  user_id: string;
  sandbox_id: string;
  repo_url: string;
  branch: string;
  model?: string;
  mode: "on_demand" | "persistent";
  status: string;
  agent_base_url?: string;
  git_policy?: CloudAgentGitPolicy;
  created_at: string;
  last_active_at: string;
}

export interface CloudAgentMessageRecord {
  id: string;
  session_id: string;
  role: string;
  content: string;
  status: string;
  created_at: string;
}

export interface CloudAgentCommitRecord {
  id: string;
  session_id: string;
  commit_sha: string;
  commit_message: string;
  branch: string;
  pushed: boolean;
  created_at: string;
}

export const authLogin = (email: string, password: string) =>
  fetchJSON<AuthTokenResponse>("/api/v1/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password }),
  });

export const authRegister = (email: string, password: string) =>
  fetchJSON<AuthTokenResponse>("/api/v1/auth/register", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password }),
  });

export const authMe = (token?: string) => {
  const headers = new Headers();
  const bearer = token ?? resolveBearerToken();
  if (bearer) headers.set("Authorization", `Bearer ${bearer}`);
  return fetchJSON<{ user: AuthUserDto }>("/api/v1/auth/me", { headers });
};

export const authOAuthStart = (provider: "google" | "github") =>
  fetchJSON<{ auth_url: string; state: string }>(
    `/api/v1/auth/oauth/${encodeURIComponent(provider)}/start`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{}",
    },
  );

export const getCloudAgents = () =>
  fetchJSON<{ sessions: CloudAgentSession[] }>("/api/v1/agents");

export const createCloudAgent = (payload: {
  repo_url: string;
  branch: string;
  model?: string;
  startup_commands?: string[];
  mode?: "on_demand" | "persistent";
  idempotency_key?: string;
}) =>
  fetchJSON<CloudAgentSession>("/api/v1/agents", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });

export const deleteCloudAgent = (id: string) =>
  fetchJSON<{ ok: boolean }>(`/api/v1/agents/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });

export const getCloudAgentStatus = (id: string) =>
  fetchJSON<{ session: CloudAgentSession; in_flight: boolean }>(
    `/api/v1/agents/${encodeURIComponent(id)}/status`,
  );

export const getCloudAgentMessages = (id: string) =>
  fetchJSON<{ messages: CloudAgentMessageRecord[] }>(
    `/api/v1/agents/${encodeURIComponent(id)}/messages`,
  );

export const sendCloudAgentMessage = (
  id: string,
  payload: { text: string; model?: string },
) =>
  fetchJSON<{ session_id: string; reply: string }>(
    `/api/v1/agents/${encodeURIComponent(id)}/messages`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    },
  );

export const interruptCloudAgent = (id: string) =>
  fetchJSON<{ ok: boolean; interrupted: boolean }>(
    `/api/v1/agents/${encodeURIComponent(id)}/interrupt`,
    { method: "POST" },
  );

export const getCloudAgentCommits = (id: string) =>
  fetchJSON<{ commits: CloudAgentCommitRecord[] }>(
    `/api/v1/agents/${encodeURIComponent(id)}/commits`,
  );

export const updateCloudAgentGitPolicy = (
  id: string,
  payload: {
    auto_commit_enabled?: boolean;
    auto_push_enabled?: boolean;
    target_branch?: string;
    protected_branches?: string[];
  },
) =>
  fetchJSON<{ session: CloudAgentSession }>(
    `/api/v1/agents/${encodeURIComponent(id)}/git-policy`,
    {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    },
  );
