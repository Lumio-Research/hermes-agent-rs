/**
 * useStreamChat — WebSocket streaming chat hook.
 *
 * Protocol:
 *  1. Client opens WS /v1/ws-stream/{sessionId}?token=<jwt>
 *  2. Server sends {"type":"connected","session_id":"..."}
 *  3. Client sends {"text":"..."}
 *  4. Server streams {"type":"text","content":"..."} chunks
 *  5. Server sends {"type":"done","content":"<full reply>"}   (or "error")
 *
 * Falls back to HTTP POST if WebSocket is unavailable.
 */

import { useState, useRef, useCallback, useEffect } from "react";
import { resolveWebSocketUrl, resolveBearerToken, apiUrl } from "../api";

interface UseStreamChatOptions {
  apiBase?: string;
}

interface StreamState {
  status: "idle" | "streaming" | "error";
  streamingContent: string;
  error?: string;
}

export function useStreamChat(_options: UseStreamChatOptions = {}) {
  const [streamState, setStreamState] = useState<StreamState>({
    status: "idle",
    streamingContent: "",
  });

  const contentRef = useRef("");
  const displayedRef = useRef("");
  const flushRafRef = useRef<number | null>(null);
  const isFlushingRef = useRef(false);
  const wsRef = useRef<WebSocket | null>(null);

  // Smoothly reveal streamed text frame-by-frame (typing effect)
  const startFlushing = useCallback(() => {
    if (isFlushingRef.current) return;
    isFlushingRef.current = true;

    const flushFrame = () => {
      if (!isFlushingRef.current) return;
      const target = contentRef.current;
      const current = displayedRef.current;
      if (target !== current) {
        if (!target.startsWith(current)) {
          displayedRef.current = target;
        } else {
          const remaining = target.length - current.length;
          const chunk = Math.max(1, Math.min(24, Math.ceil(remaining / 8)));
          displayedRef.current = current + target.slice(current.length, current.length + chunk);
        }
        setStreamState((prev) => {
          if (prev.streamingContent === displayedRef.current) return prev;
          return { ...prev, streamingContent: displayedRef.current };
        });
      }
      flushRafRef.current = requestAnimationFrame(flushFrame);
    };

    flushRafRef.current = requestAnimationFrame(flushFrame);
  }, []);

  const stopFlushing = useCallback(() => {
    isFlushingRef.current = false;
    if (flushRafRef.current !== null) {
      cancelAnimationFrame(flushRafRef.current);
      flushRafRef.current = null;
    }
    displayedRef.current = contentRef.current;
    setStreamState((prev) => ({ ...prev, streamingContent: displayedRef.current }));
  }, []);

  useEffect(() => {
    return () => {
      isFlushingRef.current = false;
      if (flushRafRef.current !== null) cancelAnimationFrame(flushRafRef.current);
      wsRef.current?.close();
    };
  }, []);

  const sendStreaming = useCallback(
    (sessionId: string, content: string): Promise<string> => {
      return new Promise((resolve, reject) => {
        contentRef.current = "";
        displayedRef.current = "";
        setStreamState({ status: "streaming", streamingContent: "" });
        startFlushing();

        const wsUrl = resolveWebSocketUrl(`/v1/ws-stream/${encodeURIComponent(sessionId)}`);
        let ws: WebSocket;

        try {
          ws = new WebSocket(wsUrl);
          wsRef.current = ws;
        } catch (err) {
          // WebSocket not available — fall back to HTTP
          _httpFallback(sessionId, content, resolve, reject);
          return;
        }

        let fullReply = "";
        let settled = false;

        const settle = (err?: Error) => {
          if (settled) return;
          settled = true;
          stopFlushing();
          try { ws.close(); } catch { /* ignore */ }
          if (err) {
            setStreamState({ status: "error", streamingContent: "", error: err.message });
            reject(err);
          } else {
            setStreamState({ status: "idle", streamingContent: "" });
            resolve(fullReply);
          }
        };

        // Timeout if no "done" arrives within 60 s
        const timeout = window.setTimeout(() => {
          settle(new Error("Agent response timed out"));
        }, 60000);

        ws.onmessage = (evt) => {
          try {
            const raw = JSON.parse(evt.data as string);
            const data = raw?.event ?? raw;
            switch (data.type) {
              case "connected":
                // Connection ready — send the message
                ws.send(JSON.stringify({ text: content }));
                break;
              case "text":
                contentRef.current += (data.content as string) ?? "";
                fullReply += (data.content as string) ?? "";
                break;
              case "thinking":
                contentRef.current += `\n> 💭 ${data.content as string}\n`;
                break;
              case "tool_start":
                contentRef.current += `\n\n🔧 **${(data.tool as string) ?? "tool"}**: ${data.content as string}\n`;
                break;
              case "tool_complete":
                contentRef.current += `\n✅ ${(data.tool as string) ?? "tool"} 完成\n`;
                break;
              case "status":
                // status events — don't append to content
                break;
              case "done":
                window.clearTimeout(timeout);
                if (data.content) fullReply = data.content as string;
                settle();
                break;
              case "error": {
                window.clearTimeout(timeout);
                const msg = (data.message ?? data.content ?? "stream error") as string;
                settle(new Error(msg));
                break;
              }
            }
          } catch {
            // ignore parse errors
          }
        };

        ws.onerror = () => {
          window.clearTimeout(timeout);
          // WS failed — fall back to HTTP
          stopFlushing();
          setStreamState({ status: "streaming", streamingContent: "" });
          startFlushing();
          settled = true;
          _httpFallback(sessionId, content, resolve, reject);
        };

        ws.onclose = () => {
          window.clearTimeout(timeout);
          if (!settled) settle();
        };
      });
    },
    [startFlushing, stopFlushing]
  );

  /** HTTP fallback when WebSocket is unavailable */
  function _httpFallback(
    sessionId: string,
    content: string,
    resolve: (v: string) => void,
    reject: (e: Error) => void
  ) {
    const token = resolveBearerToken();
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 60000);

    fetch(apiUrl(`/api/v1/sessions/${encodeURIComponent(sessionId)}/messages`), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: JSON.stringify({ text: content }),
      signal: controller.signal,
    })
      .then(async (res) => {
        const payload = await res.json().catch(() => ({}));
        if (!res.ok) throw new Error((payload as { error?: string }).error ?? `HTTP ${res.status}`);
        const reply = (payload as { reply?: string }).reply ?? "";
        contentRef.current = reply;
        return reply;
      })
      .then((reply) => {
        stopFlushing();
        setStreamState({ status: "idle", streamingContent: "" });
        resolve(reply);
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err);
        stopFlushing();
        setStreamState({ status: "error", streamingContent: "", error: message });
        reject(new Error(message));
      })
      .finally(() => window.clearTimeout(timeoutId));
  }

  const resetStream = useCallback(() => {
    contentRef.current = "";
    displayedRef.current = "";
    setStreamState({ status: "idle", streamingContent: "" });
  }, []);

  return {
    isStreaming: streamState.status === "streaming",
    streamingContent: streamState.streamingContent,
    error: streamState.error,
    sendStreaming,
    resetStream,
  };
}
