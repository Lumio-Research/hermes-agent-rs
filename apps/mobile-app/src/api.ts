// API client — adapted from web-app/src/api.ts for React Native
// Uses global fetch + WebSocket (both available in RN), no DOM/localStorage dependencies
import type { ChatMessage, Session, AppConfig } from "./types";

type ProtocolSessionSummary = {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
};

type ProtocolMessage = {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: string;
  model?: string;
};

let _apiBase = "http://127.0.0.1:8787";
let _token = "";

export function configureApi(config: Pick<AppConfig, "api_base" | "token">) {
  _apiBase = config.api_base.replace(/\/$/, "");
  _token = config.token;
}

export function getApiBase(): string {
  return _apiBase;
}

export function resolveWebSocketUrl(path: string): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  const wsBase = _apiBase.replace(/^https:/, "wss:").replace(/^http:/, "ws:");
  const url = `${wsBase}${p}`;
  return _token ? `${url}?token=${encodeURIComponent(_token)}` : url;
}

async function fetchJSON<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers as HeadersInit);
  headers.set("Content-Type", "application/json");
  if (_token) headers.set("Authorization", `Bearer ${_token}`);
  const response = await fetch(`${_apiBase}${path}`, { ...init, headers });
  if (!response.ok) {
    throw new Error(await response.text().catch(() => response.statusText));
  }
  return response.json() as Promise<T>;
}

function toSession(s: ProtocolSessionSummary): Session {
  return { id: s.id, title: s.title, created_at: s.created_at, updated_at: s.updated_at, messages: [] };
}

function toMessage(m: ProtocolMessage): ChatMessage {
  return { id: m.id, role: m.role === "tool" ? "assistant" : m.role, content: m.content, timestamp: m.timestamp, model: m.model };
}

export async function listSessions(): Promise<Session[]> {
  try {
    const res = await fetchJSON<{ sessions: ProtocolSessionSummary[] }>("/v1/sessions");
    return res.sessions.map(toSession);
  } catch {
    const res = await fetchJSON<{ sessions: ProtocolSessionSummary[] }>("/api/v1/sessions");
    return res.sessions.map(toSession);
  }
}

export async function createSession(title = "New chat"): Promise<Session> {
  try {
    const res = await fetchJSON<ProtocolSessionSummary>("/v1/sessions", {
      method: "POST",
      body: JSON.stringify({ title }),
    });
    return toSession(res);
  } catch {
    const now = new Date().toISOString();
    return { id: `local-${Date.now()}`, title, created_at: now, updated_at: now, messages: [] };
  }
}

export async function deleteSession(sessionId: string): Promise<void> {
  await fetchJSON(`/v1/sessions/${encodeURIComponent(sessionId)}`, { method: "DELETE" });
}

export async function listMessages(sessionId: string): Promise<ChatMessage[]> {
  try {
    const res = await fetchJSON<{ messages: ProtocolMessage[] }>(
      `/v1/sessions/${encodeURIComponent(sessionId)}/messages`
    );
    return res.messages.map(toMessage);
  } catch {
    const res = await fetchJSON<{ messages: ProtocolMessage[] }>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/messages`
    );
    return res.messages.map(toMessage);
  }
}

export async function checkHealth(): Promise<boolean> {
  try {
    const res = await fetch(`${_apiBase}/health`, { signal: AbortSignal.timeout(3000) });
    const data = await res.json();
    return (data as { status: string }).status === "ok";
  } catch {
    return false;
  }
}
