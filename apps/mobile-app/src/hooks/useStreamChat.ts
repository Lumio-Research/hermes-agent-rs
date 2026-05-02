// Adapted from web-app/src/hooks/useStreamChat.ts
// WebSocket is available as a global in React Native
import { useState, useRef, useCallback, useEffect } from "react";
import { resolveWebSocketUrl } from "../api";

interface StreamState {
  status: "idle" | "streaming" | "error";
  streamingContent: string;
  error?: string;
}

export function useStreamChat() {
  const [streamState, setStreamState] = useState<StreamState>({ status: "idle", streamingContent: "" });
  const contentRef = useRef("");
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    return () => { wsRef.current?.close(); };
  }, []);

  const sendStreaming = useCallback(
    (sessionId: string, content: string): Promise<string> => {
      return new Promise((resolve, reject) => {
        contentRef.current = "";
        setStreamState({ status: "streaming", streamingContent: "" });

        const url = resolveWebSocketUrl(`/v1/ws-stream/${sessionId}`);
        const ws = new WebSocket(url);
        wsRef.current = ws;
        let fullReply = "";

        ws.onmessage = (evt) => {
          try {
            const raw = JSON.parse(evt.data as string);
            const data = raw?.event ? raw.event : raw;
            switch (data.type) {
              case "connected":
                ws.send(JSON.stringify({ text: content, user_id: "mobile" }));
                break;
              case "text":
                contentRef.current += data.content;
                fullReply += data.content;
                setStreamState({ status: "streaming", streamingContent: contentRef.current });
                break;
              case "tool_start":
                contentRef.current += `\n🔧 ${String(data.tool ?? "tool")}: ${String(data.content)}\n`;
                setStreamState({ status: "streaming", streamingContent: contentRef.current });
                break;
              case "done":
                if (data.content) fullReply = data.content;
                setStreamState({ status: "idle", streamingContent: "" });
                ws.close();
                resolve(fullReply);
                break;
              case "error": {
                const msg = String(data.message ?? data.content ?? "stream error");
                setStreamState({ status: "error", streamingContent: "", error: msg });
                ws.close();
                reject(new Error(msg));
                break;
              }
            }
          } catch { /* ignore parse errors */ }
        };

        ws.onerror = () => {
          setStreamState({ status: "error", streamingContent: "", error: "WebSocket connection failed" });
          reject(new Error("WebSocket connection failed"));
        };

        ws.onclose = () => {
          if (streamState.status === "streaming") {
            setStreamState({ status: "idle", streamingContent: "" });
            resolve(fullReply);
          }
        };
      });
    },
    [streamState.status]
  );

  const resetStream = useCallback(() => {
    contentRef.current = "";
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
