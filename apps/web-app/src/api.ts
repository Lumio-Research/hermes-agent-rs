import type {
  AppConfig,
  AutomationTask,
  ChatMessage,
  PluginSettings,
  Project,
  Session,
} from "./types";

type ProtocolSessionSummary = {
  id: string;
  title?: string | null;
  created_at?: string;
  updated_at?: string;
  message_count?: number;
};

type ProtocolMessage = {
  id?: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp?: string;
  model?: string;
};

const LS_TOKEN_KEY = "hermes_api_token";
const PROJECTS_STORAGE_KEY = "hermes.browser.projects.v2";
const CONFIG_STORAGE_KEY = "hermes.browser.config.v2";
const DEFAULT_PLUGIN_SETTINGS: PluginSettings = {
  mcp_filesystem: true,
  mcp_terminal: true,
  mcp_browser: false,
  mcp_database: false,
  tool_code_exec: true,
};

function inferBrowserApiBase(): string {
  if (typeof window === "undefined") return "http://127.0.0.1:8787";
  const protocol = window.location.protocol === "https:" ? "https:" : "http:";
  const host = window.location.hostname || "127.0.0.1";
  return `${protocol}//${host}:8787`;
}

function normalizeApiBase(raw: string): string {
  return raw.replace(/\/$/, "");
}

let browserApiBase = inferBrowserApiBase();

function getApiBase(): string {
  const fromEnv =
    typeof import.meta !== "undefined" && import.meta.env?.VITE_API_BASE_URL
      ? String(import.meta.env.VITE_API_BASE_URL)
      : "";
  return normalizeApiBase(fromEnv || browserApiBase);
}

export function apiUrl(path: string): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  return `${getApiBase()}${p}`;
}

export function resolveBearerToken(): string | null {
  try {
    const v = localStorage.getItem(LS_TOKEN_KEY);
    return v && v.trim() ? v.trim() : null;
  } catch {
    return null;
  }
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

async function fetchVoid(path: string, init?: RequestInit): Promise<void> {
  const headers = new Headers(init?.headers);
  const token = resolveBearerToken();
  if (token && !headers.has("Authorization")) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  const response = await fetch(apiUrl(path), { ...init, headers });
  if (!response.ok) {
    throw new Error(await response.text().catch(() => response.statusText));
  }
}

function toSession(summary: ProtocolSessionSummary): Session {
  const now = new Date().toISOString();
  return {
    id: summary.id,
    title: summary.title?.trim() || "New chat",
    created_at: summary.created_at ?? now,
    updated_at: summary.updated_at ?? summary.created_at ?? now,
    project: undefined,
    messages: [],
  };
}

function toChatMessage(msg: ProtocolMessage, fallbackId: string): ChatMessage | null {
  // Filter out system messages and tool calls — these are internal
  // configuration/context injections that should not be shown to the user.
  if (msg.role === "system" || msg.role === "tool") return null;
  return {
    id: msg.id ?? fallbackId,
    role: msg.role as "user" | "assistant",
    content: msg.content,
    timestamp: msg.timestamp ?? new Date().toISOString(),
    model: msg.model,
  };
}

export const getSessions = async (): Promise<Session[]> => {
  const response = await fetchJSON<{ sessions: ProtocolSessionSummary[] }>("/api/v1/sessions");
  return response.sessions.map(toSession);
};

export const createSession = async (title: string, project?: string): Promise<Session> => {
  try {
    const response = await fetchJSON<ProtocolSessionSummary>("/api/v1/sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title, project }),
    });
    return toSession(response);
  } catch (e) {
    // Backward compatibility: older cloud backends only create sessions lazily on first message.
    if (String(e).includes("404") || String(e).includes("405")) {
      const now = new Date().toISOString();
      return {
        id: crypto.randomUUID(),
        title: title.trim() || "New chat",
        created_at: now,
        updated_at: now,
        project,
        messages: [],
      };
    }
    throw e;
  }
};

export const deleteSession = async (sessionId: string): Promise<boolean> => {
  await fetchVoid(`/api/v1/sessions/${encodeURIComponent(sessionId)}`, { method: "DELETE" });
  return true;
};

export const renameSession = async (sessionId: string, title: string): Promise<boolean> => {
  await fetchVoid(`/api/v1/sessions/${encodeURIComponent(sessionId)}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ title }),
  });
  return true;
};

export const getSessionMessages = async (sessionId: string): Promise<ChatMessage[]> => {
  const response = await fetchJSON<{ messages: ProtocolMessage[] }>(
    `/api/v1/sessions/${encodeURIComponent(sessionId)}/messages`,
  );
  return response.messages
    .map((m, index) => toChatMessage(m, `${sessionId}-${index}`))
    .filter((m): m is ChatMessage => m !== null);
};

export const sendMessage = async (sessionId: string, content: string): Promise<ChatMessage> => {
  await fetchJSON<{ session_id: string; reply: string; message_count: number }>(
    `/api/v1/sessions/${encodeURIComponent(sessionId)}/messages`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        text: content,
        request_id: `req-${Date.now()}`,
      }),
    },
  );
  return {
    id: `sent-${Date.now()}`,
    role: "user",
    content,
    timestamp: new Date().toISOString(),
  };
};

export const sendMessageStream = (sessionId: string, content: string) =>
  sendMessage(sessionId, content);

function loadProjects(): Project[] {
  try {
    const raw = localStorage.getItem(PROJECTS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveProjects(projects: Project[]) {
  try {
    localStorage.setItem(PROJECTS_STORAGE_KEY, JSON.stringify(projects));
  } catch {
    // ignore
  }
}

let localProjects = loadProjects();
let localConfig: AppConfig = (() => {
  try {
    const raw = localStorage.getItem(CONFIG_STORAGE_KEY);
    if (!raw) {
      return {
        api_base: browserApiBase,
        default_model: "",
        theme: "dark",
        mode: "remote",
      };
    }
    const parsed = JSON.parse(raw) as Partial<AppConfig>;
    const cfg: AppConfig = {
      api_base: parsed.api_base || browserApiBase,
      default_model: parsed.default_model || "",
      theme: parsed.theme || "dark",
      mode: "remote",
    };
    browserApiBase = normalizeApiBase(cfg.api_base);
    return cfg;
  } catch {
    return {
      api_base: browserApiBase,
      default_model: "",
      theme: "dark",
      mode: "remote",
    };
  }
})();

function saveConfig(config: AppConfig) {
  try {
    localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(config));
  } catch {
    // ignore
  }
}

export const getProjects = async () => localProjects;
export const addProject = async (name: string, path: string) => {
  const project: Project = {
    id: `project-${Date.now()}`,
    name,
    path,
  };
  localProjects = [...localProjects, project];
  saveProjects(localProjects);
  return project;
};
export const removeProject = async (projectId: string) => {
  localProjects = localProjects.filter((p) => p.id !== projectId);
  saveProjects(localProjects);
  return true;
};
export const getConfig = async () => localConfig;
export const updateConfig = async (config: AppConfig) => {
  localConfig = { ...config, mode: "remote", api_base: normalizeApiBase(config.api_base) };
  browserApiBase = localConfig.api_base;
  saveConfig(localConfig);
  return localConfig;
};
export const getAutomationTasks = async (): Promise<AutomationTask[]> => [
  { id: "1", title: "Summarize yesterday's git activity for standup.", description: "Status reports", icon: "📝", category: "Status reports" },
  { id: "2", title: "Synthesize this week's PRs, rollouts, incidents, and reviews into a weekly update.", description: "Status reports", icon: "📋", category: "Status reports" },
  { id: "3", title: "Summarize last week's PRs by teammate and theme; highlight risks.", description: "Status reports", icon: "🖥️", category: "Status reports" },
  { id: "4", title: "Draft weekly release notes from merged PRs (include links when available).", description: "Release prep", icon: "📮", category: "Release prep" },
  { id: "5", title: "Before tagging, verify changelog, migrations, feature flags, and tests.", description: "Release prep", icon: "✅", category: "Release prep" },
  { id: "6", title: "Update the changelog with this week's highlights and key PR links.", description: "Release prep", icon: "✏️", category: "Release prep" },
  { id: "7", title: "Triage new issues: label, assign, and flag anything blocking.", description: "Incidents & triage", icon: "🔍", category: "Incidents & triage" },
  { id: "8", title: "Summarize open incidents and their current status.", description: "Incidents & triage", icon: "💬", category: "Incidents & triage" },
];
export const checkBackendHealth = async (): Promise<boolean> => {
  try {
    const resp = await fetch(apiUrl("/health"), { signal: AbortSignal.timeout(3000) });
    const data = await resp.json();
    return data.status === "ok";
  } catch {
    return false;
  }
};
export const execCommand = async (_command: string, _sessionId?: string) => "Unsupported in protocol client";

type RemotePluginPayload = {
  plugins?: PluginSettings;
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

export async function getPluginSettings(): Promise<PluginSettings> {
  try {
    const payload = await fetchJSON<RemotePluginPayload>("/api/dashboard/plugins");
    return normalizePluginSettings(payload.plugins);
  } catch {
    return { ...DEFAULT_PLUGIN_SETTINGS };
  }
}

export async function updatePluginSettings(plugins: PluginSettings): Promise<PluginSettings> {
  const normalized = normalizePluginSettings(plugins);
  try {
    const payload = await fetchJSON<RemotePluginPayload>("/api/dashboard/plugins", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ plugins: normalized }),
    });
    return normalizePluginSettings(payload.plugins ?? normalized);
  } catch {
    return normalized;
  }
}

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
  client_session_id?: string;
  sandbox_id: string;
  repo_url: string;
  branch: string;
  model?: string;
  mode: "on_demand" | "persistent";
  workspace_mode?: "repo" | "blank";
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
  tool_calls?: Array<{ name: string; status: "running" | "done" | "error"; output?: string }>;
  execution_timeline?: Array<{
    type: "tool_start" | "tool_stdout" | "tool_complete" | "status";
    tool?: string;
    content?: string;
    arguments?: string;
    chunk_index?: number;
    chunk_total?: number;
    created_at: string;
  }>;
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
  repo_url?: string;
  client_session_id?: string;
  branch?: string;
  workspace_mode?: "repo" | "blank";
  sandbox_backend?: "docker" | "cloudflare" | "modal" | "runloop" | "fly";
  model?: string;
  execution_profile?: "tool_use_strong" | "balanced" | "cheap_fast";
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
  payload: {
    text: string;
    model?: string;
    execution_profile?: "tool_use_strong" | "balanced" | "cheap_fast";
  },
) =>
  fetchJSON<{ session_id: string; reply: string }>(
    `/api/v1/agents/${encodeURIComponent(id)}/messages`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    },
  );

export const sendCloudAgentMessageStream = (
  id: string,
  payload: {
    text: string;
    model?: string;
    execution_profile?: "tool_use_strong" | "balanced" | "cheap_fast";
  },
  handlers: {
    onChunk?: (text: string) => void;
    onToolCall?: (tool: { id?: string; name?: string; arguments?: string }) => void;
    onToolStart?: (tool: string, content?: string, args?: string) => void;
    onToolStdout?: (tool: string, content?: string, chunkIndex?: number, chunkTotal?: number) => void;
    onToolComplete?: (tool: string, content?: string) => void;
    onStatus?: (content: string, kind?: string) => void;
    onDone?: (fullText: string) => void;
    onError?: (error: string) => void;
  } = {},
): Promise<{ session_id: string; reply: string }> =>
  new Promise((resolve, reject) => {
    const wsUrl = resolveWebSocketUrl(`/api/v1/agents/${encodeURIComponent(id)}/ws`);
    let ws: WebSocket;
    let full = "";
    let done = false;

    const settleError = (msg: string) => {
      if (done) return;
      done = true;
      handlers.onError?.(msg);
      reject(new Error(msg));
      try {
        ws.close();
      } catch {
        // ignore
      }
    };

    const settleDone = (text: string) => {
      if (done) return;
      done = true;
      handlers.onDone?.(text);
      resolve({ session_id: id, reply: text });
      try {
        ws.close();
      } catch {
        // ignore
      }
    };

    try {
      ws = new WebSocket(wsUrl);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      settleError(msg);
      return;
    }

    ws.onmessage = (evt) => {
      try {
        const data = JSON.parse(String(evt.data));
        if (data?.type === "connected") {
          ws.send(JSON.stringify(payload));
          return;
        }
        if (data?.type === "chunk") {
          const piece = data?.chunk?.delta?.content;
          if (typeof piece === "string" && piece.length > 0) {
            full += piece;
            handlers.onChunk?.(piece);
          }
          const toolCalls = data?.chunk?.delta?.tool_calls;
          if (Array.isArray(toolCalls)) {
            for (const tc of toolCalls) {
              handlers.onToolCall?.({
                id: tc?.id,
                name: tc?.function?.name,
                arguments: tc?.function?.arguments,
              });
            }
          }
          return;
        }
        if (data?.type === "tool_start") {
          const args = data?.arguments;
          handlers.onToolStart?.(
            String(data?.tool ?? "tool"),
            String(data?.content ?? ""),
            typeof args === "string" ? args : JSON.stringify(args ?? {})
          );
          return;
        }
        if (data?.type === "tool_stdout") {
          const chunkIndex = Number(data?.chunk_index);
          const chunkTotal = Number(data?.chunk_total);
          handlers.onToolStdout?.(
            String(data?.tool ?? "tool"),
            String(data?.content ?? ""),
            Number.isFinite(chunkIndex) ? chunkIndex : undefined,
            Number.isFinite(chunkTotal) ? chunkTotal : undefined
          );
          return;
        }
        if (data?.type === "tool_complete") {
          handlers.onToolComplete?.(String(data?.tool ?? "tool"), String(data?.content ?? ""));
          return;
        }
        if (data?.type === "status") {
          handlers.onStatus?.(String(data?.content ?? ""), String(data?.kind ?? "lifecycle"));
          return;
        }
        if (data?.type === "done") {
          const text = typeof data?.content === "string" ? data.content : full;
          settleDone(text);
          return;
        }
        if (data?.type === "error") {
          settleError(data?.content || "cloud agent stream error");
        }
      } catch {
        // ignore malformed frames
      }
    };

    ws.onerror = () => {
      if (done) return;
      settleError("cloud agent websocket error");
    };

    ws.onclose = () => {
      if (done) return;
      // If server closed without explicit done, return what we already got.
      if (full.length > 0) {
        settleDone(full);
      } else {
        settleError("cloud agent websocket closed unexpectedly");
      }
    };
  });

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
