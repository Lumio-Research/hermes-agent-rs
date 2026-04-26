/**
 * useStreamChat — React hook for streaming chat with WebSocket or Tauri events.
 *
 * Follows the Vercel AI SDK useChat pattern:
 * - Manages messages state
 * - Handles streaming token-by-token
 * - Batches state updates to avoid layout thrash
 * - Works in both Tauri (native events) and browser (WebSocket) modes
 */

import { useState, useRef, useCallback, useEffect } from "react";
import type { } from "../types";

interface UseStreamChatOptions {
  /** hermes-http API base URL for browser WebSocket mode */
  apiBase?: string;
}

interface StreamState {
  status: "idle" | "streaming" | "error";
  streamingContent: string;
  error?: string;
}

export function useStreamChat(options: UseStreamChatOptions = {}) {
  const { apiBase = "http://127.0.0.1:8787" } = options;

  const [streamState, setStreamState] = useState<StreamState>({
    status: "idle",
    streamingContent: "",
  });

  // Use a ref to accumulate tokens without triggering re-renders on every token
  const contentRef = useRef("");
  const flushTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Flush accumulated content to state every 50ms (batched updates)
  const startFlushing = useCallback(() => {
    if (flushTimerRef.current) return;
    flushTimerRef.current = setInterval(() => {
      setStreamState((prev) => {
        if (prev.streamingContent === contentRef.current) return prev;
        return { ...prev, streamingContent: contentRef.current };
      });
    }, 50);
  }, []);

  const stopFlushing = useCallback(() => {
    if (flushTimerRef.current) {
      clearInterval(flushTimerRef.current);
      flushTimerRef.current = null;
    }
    // Final flush
    setStreamState((prev) => ({
      ...prev,
      streamingContent: contentRef.current,
    }));
  }, []);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (flushTimerRef.current) clearInterval(flushTimerRef.current);
      wsRef.current?.close();
    };
  }, []);

  /**
   * Send a message and stream the response via WebSocket.
   * Returns the final complete reply text.
   */
  const sendStreaming = useCallback(
    (sessionId: string, content: string): Promise<string> => {
      return new Promise((resolve, reject) => {
        // Reset state
        contentRef.current = "";
        setStreamState({ status: "streaming", streamingContent: "" });
        startFlushing();

        const wsBase = apiBase.replace("http://", "ws://").replace("https://", "wss://");
        const url = `${wsBase}/v1/ws-stream/${sessionId}`;

        const ws = new WebSocket(url);
        wsRef.current = ws;
        let fullReply = "";

        ws.onmessage = (evt) => {
          try {
            const data = JSON.parse(evt.data);

            switch (data.type) {
              case "connected":
                ws.send(JSON.stringify({ text: content, user_id: "app" }));
                break;

              case "text":
                contentRef.current += data.content;
                fullReply += data.content;
                break;

              case "thinking":
                contentRef.current += `\n> 💭 ${data.content}`;
                break;

              case "tool_start":
                contentRef.current += `\n\n🔧 **${data.tool || "tool"}**: ${data.content}\n`;
                break;

              case "tool_complete":
                contentRef.current += `\n✅ ${data.tool || "tool"} 完成\n`;
                break;

              case "status":
                contentRef.current += `\n_${data.content}_\n`;
                break;

              case "activity":
                contentRef.current += `\n⏳ ${data.content}\n`;
                break;

              case "done":
                stopFlushing();
                if (data.content) fullReply = data.content;
                setStreamState({
                  status: "idle",
                  streamingContent: "",
                });
                ws.close();
                resolve(fullReply);
                break;

              case "error":
                stopFlushing();
                setStreamState({
                  status: "error",
                  streamingContent: "",
                  error: data.content,
                });
                ws.close();
                reject(new Error(data.content));
                break;
            }
          } catch {
            // ignore parse errors
          }
        };

        ws.onerror = () => {
          stopFlushing();
          setStreamState({ status: "error", streamingContent: "", error: "WebSocket connection failed" });
          reject(new Error("WebSocket connection failed"));
        };

        ws.onclose = () => {
          stopFlushing();
          if (streamState.status === "streaming") {
            setStreamState({ status: "idle", streamingContent: "" });
            resolve(fullReply);
          }
        };
      });
    },
    [apiBase, startFlushing, stopFlushing, streamState.status]
  );

  /**
   * Handle a Tauri stream-delta event (for native mode).
   */
  const handleTauriDelta = useCallback(
    (deltaType: string, content: string) => {
      if (streamState.status !== "streaming" && deltaType !== "done") {
        contentRef.current = "";
        setStreamState({ status: "streaming", streamingContent: "" });
        startFlushing();
      }

      switch (deltaType) {
        case "text":
          contentRef.current += content;
          break;
        case "thinking":
          contentRef.current += `\n> 💭 ${content}`;
          break;
        case "tool_start":
          contentRef.current += `\n\n🔧 ${content}\n`;
          break;
        case "tool_complete":
          contentRef.current += `\n✅ ${content}\n`;
          break;
        case "status":
          contentRef.current += `\n_${content}_\n`;
          break;
        case "activity":
          contentRef.current += `\n⏳ ${content}\n`;
          break;
        case "done":
          stopFlushing();
          setStreamState({ status: "idle", streamingContent: "" });
          break;
      }
    },
    [startFlushing, stopFlushing, streamState.status]
  );

  return {
    /** Current streaming status */
    isStreaming: streamState.status === "streaming",
    /** Accumulated streaming content (updates every 50ms during streaming) */
    streamingContent: streamState.streamingContent,
    /** Error message if streaming failed */
    error: streamState.error,
    /** Send a message via WebSocket streaming (browser mode) */
    sendStreaming,
    /** Handle a Tauri stream-delta event (native mode) */
    handleTauriDelta,
  };
}
