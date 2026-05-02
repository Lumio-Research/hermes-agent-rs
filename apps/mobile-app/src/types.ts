// Shared types — mirrors web-app/src/types.ts
export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: string;
  model?: string;
}

export interface Session {
  id: string;
  title: string;
  messages: ChatMessage[];
  created_at: string;
  updated_at: string;
}

export interface AppConfig {
  api_base: string;
  token: string;
  default_model: string;
}
