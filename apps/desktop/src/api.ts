import type {
  Session,
  ChatMessage,
  Project,
  AutomationTask,
  AppConfig,
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
let browserApiBase = "http://127.0.0.1:8787";

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
    return tauriInvoke<T>(cmd, args);
  }
  // Browser mode: use hermes-dashboard API for messages, mock for UI state
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
          if (session) session.messages.push(assistantMsg);
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
          if (session) session.messages.push(assistantMsg);
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
  api_base: "http://127.0.0.1:8787",
  default_model: "",
  theme: "dark",
  mode: "remote",  // Browser always uses remote mode
};

let idCounter = 1;
function uid() {
  return `mock-${Date.now()}-${idCounter++}`;
}

async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const now = new Date().toISOString();
  switch (cmd) {
    case "get_sessions": return mockSessions as T;
    case "create_session": {
      const s: Session = { id: uid(), title: (args?.title as string) || "新聊天", project: args?.project as string | undefined, messages: [], created_at: now, updated_at: now };
      mockSessions = [s, ...mockSessions];
      return s as T;
    }
    case "delete_session": { mockSessions = mockSessions.filter(s => s.id !== args?.sessionId); return true as T; }
    case "rename_session": {
      const s = mockSessions.find(s => s.id === args?.sessionId);
      if (s) { s.title = args?.title as string; s.updated_at = now; }
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
        return userMsg as T;
      }
      return null as T;
    }
    case "get_projects": return mockProjects as T;
    case "add_project": { const p: Project = { id: uid(), name: args?.name as string, path: args?.path as string }; mockProjects.push(p); return p as T; }
    case "remove_project": { mockProjects = mockProjects.filter(p => p.id !== args?.projectId); return true as T; }
    case "get_config": return mockConfig as T;
    case "update_config": { mockConfig = args?.config as AppConfig; return mockConfig as T; }
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
