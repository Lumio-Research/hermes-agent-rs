export type StreamEvent =
  | { type: "connected"; session_id: string }
  | { type: "text"; content: string }
  | { type: "thinking"; content: string }
  | { type: "tool_start"; tool: string; content: string }
  | { type: "tool_complete"; tool: string; content: string }
  | { type: "status"; content: string }
  | { type: "activity"; content: string }
  | { type: "error"; code: string; message: string }
  | { type: "done"; content: string };

export type WsEnvelope = {
  version: number;
  request_id?: string;
  trace_id: string;
  event: StreamEvent;
};

export type SessionSummary = {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
};

export type ProtocolMessage = {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: string;
  model?: string;
};

export class HermesAppSdk {
  constructor(
    private readonly apiBase: string,
    private readonly token?: string,
  ) {}

  private authHeaders(): HeadersInit {
    if (!this.token) return {};
    return { Authorization: `Bearer ${this.token}` };
  }

  async createSession(title: string, project?: string): Promise<SessionSummary> {
    const res = await fetch(`${this.apiBase}/v1/sessions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...this.authHeaders(),
      },
      body: JSON.stringify({ title, project }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  }

  async listMessages(sessionId: string): Promise<ProtocolMessage[]> {
    const res = await fetch(
      `${this.apiBase}/v1/sessions/${encodeURIComponent(sessionId)}/messages`,
      { headers: this.authHeaders() },
    );
    if (!res.ok) throw new Error(await res.text());
    const payload = (await res.json()) as { messages: ProtocolMessage[] };
    return payload.messages;
  }

  interrupt(_sessionId: string): Promise<void> {
    // Protocol keeps interrupt extensible. Current server does not expose a hard interrupt endpoint yet.
    return Promise.resolve();
  }

  async *sendMessageStream(
    sessionId: string,
    text: string,
    requestId = `req-${Date.now()}`,
  ): AsyncGenerator<WsEnvelope, void, unknown> {
    const wsBase = this.apiBase.replace("http://", "ws://").replace("https://", "wss://");
    const wsUrl = new URL(`${wsBase}/v1/ws-stream/${encodeURIComponent(sessionId)}`);
    if (this.token) {
      wsUrl.searchParams.set("token", this.token);
    }
    const ws = new WebSocket(wsUrl.toString());
    await new Promise<void>((resolve, reject) => {
      ws.onopen = () => resolve();
      ws.onerror = () => reject(new Error("WebSocket open failed"));
    });

    const queue: WsEnvelope[] = [];
    let closed = false;
    let error: Error | null = null;
    let wake: (() => void) | null = null;

    ws.onmessage = (evt) => {
      try {
        const payload = JSON.parse(String(evt.data)) as WsEnvelope;
        queue.push(payload);
        if (wake) wake();
      } catch {
        // ignore malformed event
      }
    };
    ws.onerror = () => {
      error = new Error("WebSocket stream failed");
      if (wake) wake();
    };
    ws.onclose = () => {
      closed = true;
      if (wake) wake();
    };

    ws.send(
      JSON.stringify({
        text,
        user_id: "sdk",
        request_id: requestId,
      }),
    );

    try {
      while (!closed || queue.length > 0) {
        if (error) throw error;
        if (queue.length === 0) {
          await new Promise<void>((resolve) => {
            wake = () => {
              wake = null;
              resolve();
            };
          });
          continue;
        }
        const next = queue.shift() as WsEnvelope;
        yield next;
        if (next.event.type === "done" || next.event.type === "error") {
          ws.close();
        }
      }
    } finally {
      try {
        ws.close();
      } catch {
        // ignore close errors on early consumer cancellation
      }
    }
  }
}
