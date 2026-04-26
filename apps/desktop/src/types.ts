export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: string;
  model?: string;
  tool_calls?: ToolCall[];
}

export interface ToolCall {
  name: string;
  status: "running" | "done" | "error";
  output?: string;
}

export interface Session {
  id: string;
  title: string;
  project?: string;
  messages: ChatMessage[];
  created_at: string;
  updated_at: string;
}

export interface Project {
  id: string;
  name: string;
  path: string;
}

export interface AutomationTask {
  id: string;
  title: string;
  description: string;
  icon: string;
  category: string;
}

export interface AppConfig {
  api_base: string;
  default_model: string;
  theme: string;
  mode: "local" | "remote";
}

export type NavPage = "chat" | "search" | "plugins" | "automation" | "settings";
