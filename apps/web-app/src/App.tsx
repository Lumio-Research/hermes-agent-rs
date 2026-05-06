import { useState, useEffect, useCallback, useRef } from "react";
import { Sidebar } from "./components/Sidebar";
import { ChatView } from "./components/ChatView";
import { AutomationView } from "./components/AutomationView";
import { SettingsView } from "./components/SettingsView";
import { SearchView } from "./components/SearchView";
import { PluginsView } from "./components/PluginsView";
import { TitleBar } from "./components/TitleBar";
import { RightPanel } from "./components/RightPanel";
import { useStreamChat } from "./hooks/useStreamChat";
import * as api from "./api";
import type { Session, Project, NavPage, ChatMessage } from "./types";

export default function App() {
  const SIDEBAR_WIDTH_KEY = "hermes.ui.sidebar.width.v1";
  const CLOUD_AGENT_MAP_KEY = "hermes.ui.cloud_agent_by_session.v1";
  const CLOUD_AGENT_MODEL = String(import.meta.env.VITE_CLOUD_AGENT_MODEL ?? "").trim() || undefined;
  const CLOUD_AGENT_EXECUTION_PROFILE = (
    String(import.meta.env.VITE_CLOUD_AGENT_EXECUTION_PROFILE ?? "").trim().toLowerCase() ||
    "tool_use_strong"
  ) as "tool_use_strong" | "balanced" | "cheap_fast";
  const SIDEBAR_MIN_WIDTH = 200;
  const SIDEBAR_MAX_WIDTH = 360;
  const [page, setPage] = useState<NavPage>("chat");
  const [sessions, setSessions] = useState<Session[]>([]);
  const ACTIVE_SESSION_KEY = "hermes.ui.active_session_id.v1";
  const [activeSessionId, setActiveSessionIdRaw] = useState<string | null>(() => {
    try {
      return window.localStorage.getItem(ACTIVE_SESSION_KEY);
    } catch {
      return null;
    }
  });
  const setActiveSessionId = useCallback((id: string | null) => {
    setActiveSessionIdRaw(id);
    try {
      if (id) {
        window.localStorage.setItem(ACTIVE_SESSION_KEY, id);
      } else {
        window.localStorage.removeItem(ACTIVE_SESSION_KEY);
      }
    } catch {
      /* localStorage unavailable; in-memory only */
    }
  }, []);
  const [projects, setProjects] = useState<Project[]>([]);
  const [cloudAgentBySession, setCloudAgentBySession] = useState<Record<string, string>>(() => {
    if (typeof window === "undefined") return {};
    try {
      const raw = window.localStorage.getItem(CLOUD_AGENT_MAP_KEY);
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      return parsed && typeof parsed === "object" ? (parsed as Record<string, string>) : {};
    } catch {
      return {};
    }
  });
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    if (typeof window === "undefined") return 232;
    const raw = window.localStorage.getItem(SIDEBAR_WIDTH_KEY);
    const n = raw ? Number(raw) : NaN;
    return Number.isFinite(n) ? Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, n)) : 232;
  });
  const resizingSidebarRef = useRef(false);
  const loadedSessionMessagesRef = useRef<Set<string>>(new Set());

  const { isStreaming, streamingContent, sendStreaming, resetStream } = useStreamChat();

  // Auto-collapse sidebar on small screens
  useEffect(() => {
    const mq = window.matchMedia("(max-width: 768px)");
    const handler = (e: MediaQueryListEvent | MediaQueryList) => setSidebarOpen(!e.matches);
    handler(mq);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  useEffect(() => {
    api.getSessions().then(setSessions);
    api.getProjects().then(setProjects);
  }, []);

  useEffect(() => {
    if (!activeSessionId) return;
    const session = sessions.find((s) => s.id === activeSessionId);
    if (!session) {
      if (sessions.length > 0) setActiveSessionId(null);
      return;
    }
    if (session.messages.length > 0 || loadedSessionMessagesRef.current.has(activeSessionId)) {
      return;
    }
    loadedSessionMessagesRef.current.add(activeSessionId);
    api.getSessionMessages(activeSessionId)
      .then((messages) => {
        setSessions((prev) =>
          prev.map((s) => (s.id === activeSessionId ? { ...s, messages } : s))
        );
      })
      .catch(() => {
        // Leave the session selectable even if the backend has no messages yet.
      });
  }, [activeSessionId, sessions, setActiveSessionId]);

  useEffect(() => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
    }
  }, [sidebarWidth]);

  useEffect(() => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(CLOUD_AGENT_MAP_KEY, JSON.stringify(cloudAgentBySession));
    }
  }, [cloudAgentBySession]);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!resizingSidebarRef.current) return;
      const next = Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, e.clientX));
      setSidebarWidth(next);
    };
    const onUp = () => {
      if (!resizingSidebarRef.current) return;
      resizingSidebarRef.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, []);

  const activeSession = sessions.find((s) => s.id === activeSessionId) ?? null;
  const activeSessionCloudAgentId = activeSessionId ? cloudAgentBySession[activeSessionId] : undefined;

  const extractGithubRepoUrl = useCallback((text: string): string | null => {
    const m = text.match(/https?:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:\.git)?/i);
    return m ? m[0] : null;
  }, []);

  const enforceSandboxExecution = useCallback((text: string, repoUrl: string | null): string => {
    const isRepoAnalysis = Boolean(repoUrl) || /(分析|analy[sz]e|项目|仓库|repo|codebase)/i.test(text);
    if (!isRepoAnalysis) return text;
    return [
      "请在本次回答中必须实际执行终端命令，不要只给计划。",
      "执行要求：",
      "1) 先执行 pwd、ls、git status 进行环境确认；",
      "2) 如果是仓库分析，至少再执行 2 条与结构相关命令（例如 tree/find/cargo metadata 等）；",
      "3) 回答中必须附上关键命令输出片段，并基于输出下结论；",
      "4) 禁止只返回“你可以执行这些命令”。",
      "",
      "用户原始请求：",
      text,
    ].join("\n");
  }, []);

  const shouldUseSandboxAutoRoute = useCallback(
    (text: string) => {
      if (extractGithubRepoUrl(text)) return true;
      return /(分析|analy[sz]e|项目|仓库|repo|codebase)/i.test(text);
    },
    [extractGithubRepoUrl]
  );

  const handleNewChat = useCallback(async () => {
    const session = await api.createSession("New chat");
    setSessions((prev) => [session, ...prev]);
    setActiveSessionId(session.id);
    setPage("chat");
  }, []);

  const handleSelectSession = useCallback(async (id: string) => {
    setActiveSessionId(id);
    setPage("chat");
    if (window.innerWidth < 768) setSidebarOpen(false);
    // Load messages from backend if not already fetched
    setSessions((prev) => {
      const session = prev.find((s) => s.id === id);
      if (session && session.messages.length > 0) return prev;
      // Trigger async fetch outside of this updater
      return prev;
    });
    try {
      const messages = await api.getSessionMessages(id);
      setSessions((prev) =>
        prev.map((s) => (s.id === id ? { ...s, messages } : s))
      );
    } catch {
      // silently ignore - session might be new or backend unavailable
    }
  }, []);

  const handleDeleteSession = useCallback(
    async (id: string) => {
      await api.deleteSession(id);
      setSessions((prev) => prev.filter((s) => s.id !== id));
      if (activeSessionId === id) setActiveSessionId(null);
    },
    [activeSessionId]
  );

  const handleSendMessage = useCallback(
    async (content: string, projectId?: string) => {
      let sessionId = activeSessionId;
      if (sessionId && !sessions.some((s) => s.id === sessionId)) {
        sessionId = null;
        setActiveSessionId(null);
      }
      if (!sessionId) {
        const project = projectId ? projects.find((p) => p.id === projectId) : undefined;
        const title = content.slice(0, 40) + (content.length > 40 ? "..." : "");
        const session = await api.createSession(title, project?.path);
        setSessions((prev) => [session, ...prev]);
        setActiveSessionId(session.id);
        sessionId = session.id;
      }

      // Add user message to UI immediately
      const userMsg: ChatMessage = {
        id: `temp-${Date.now()}`,
        role: "user",
        content,
        timestamp: new Date().toISOString(),
      };
      setSessions((prev) =>
        prev.map((s) =>
          s.id === sessionId
            ? {
                ...s,
                messages: [...s.messages, userMsg],
                updated_at: new Date().toISOString(),
                title: s.messages.length === 0
                  ? content.slice(0, 30) + (content.length > 30 ? "..." : "")
                  : s.title,
              }
            : s
        )
      );

      try {
        const sessionCloudAgentId = cloudAgentBySession[sessionId];
        const repoUrl = extractGithubRepoUrl(content);
        const routeToSandbox = Boolean(sessionCloudAgentId) || shouldUseSandboxAutoRoute(content);

        if (routeToSandbox) {
          let cloudAgentId = sessionCloudAgentId;
          if (!cloudAgentId) {
            const created = await api.createCloudAgent(
              repoUrl
                ? {
                    repo_url: repoUrl,
                    client_session_id: sessionId,
                    workspace_mode: "repo",
                    mode: "on_demand",
                    execution_profile: CLOUD_AGENT_EXECUTION_PROFILE,
                  }
                : {
                    client_session_id: sessionId,
                    workspace_mode: "blank",
                    mode: "on_demand",
                    execution_profile: CLOUD_AGENT_EXECUTION_PROFILE,
                  }
            );
            cloudAgentId = created.id;
            setCloudAgentBySession((prev) => ({ ...prev, [sessionId]: created.id }));
          }

          const cloudPrompt = enforceSandboxExecution(content, repoUrl);
          const streamingId = `reply-${Date.now()}`;
          const startTs = new Date().toISOString();
          setSessions((prev) =>
            prev.map((s) =>
              s.id === sessionId
                ? {
                    ...s,
                    messages: [
                      ...s.messages,
                      {
                        id: streamingId,
                        role: "assistant",
                        content: "",
                        timestamp: startTs,
                        model: "cloud-agent",
                        execution_backend: "sandbox",
                      },
                    ],
                    updated_at: startTs,
                  }
                : s
            )
          );

          await api.sendCloudAgentMessageStream(
            cloudAgentId,
            CLOUD_AGENT_MODEL
              ? { text: cloudPrompt, model: CLOUD_AGENT_MODEL }
              : { text: cloudPrompt, execution_profile: CLOUD_AGENT_EXECUTION_PROFILE },
            {
            onChunk: (piece) => {
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sessionId
                    ? {
                        ...s,
                        messages: s.messages.map((m) =>
                          m.id === streamingId ? { ...m, content: `${m.content}${piece}` } : m
                        ),
                        updated_at: new Date().toISOString(),
                      }
                    : s
                )
              );
            },
            onToolCall: (tool) => {
              const toolName = tool.name?.trim() || "tool";
              const ts = new Date().toISOString();
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sessionId
                    ? {
                        ...s,
                        messages: s.messages.map((m) => {
                          if (m.id !== streamingId) return m;
                          const current = m.tool_calls ?? [];
                          if (current.some((tc) => tc.name === toolName && tc.status !== "error")) {
                            return m;
                          }
                          return {
                            ...m,
                            tool_calls: [...current, { name: toolName, status: "running", output: tool.arguments }],
                            execution_timeline: [
                              ...(m.execution_timeline ?? []),
                              {
                                type: "tool_start",
                                tool: toolName,
                                arguments: tool.arguments,
                                created_at: ts,
                              },
                            ],
                          };
                        }),
                        updated_at: new Date().toISOString(),
                      }
                    : s
                )
              );
            },
            onToolStart: (tool, content, args) => {
              const ts = new Date().toISOString();
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sessionId
                    ? {
                        ...s,
                        messages: s.messages.map((m) =>
                          m.id === streamingId
                            ? {
                                ...m,
                                tool_calls: [
                                  ...(m.tool_calls ?? []),
                                  { name: tool, status: "running", output: content || undefined },
                                ],
                                execution_timeline: [
                                  ...(m.execution_timeline ?? []),
                                  {
                                    type: "tool_start",
                                    tool,
                                    arguments: args,
                                    content: content || undefined,
                                    created_at: ts,
                                  },
                                ],
                              }
                            : m
                        ),
                      }
                    : s
                )
              );
            },
            onToolStdout: (tool, content, chunkIndex, chunkTotal) => {
              const ts = new Date().toISOString();
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sessionId
                    ? {
                        ...s,
                        messages: s.messages.map((m) =>
                          m.id === streamingId
                            ? {
                                ...m,
                                execution_timeline: [
                                  ...(m.execution_timeline ?? []),
                                  {
                                    type: "tool_stdout",
                                    tool,
                                    content: content || undefined,
                                    chunk_index: chunkIndex,
                                    chunk_total: chunkTotal,
                                    created_at: ts,
                                  },
                                ],
                              }
                            : m
                        ),
                      }
                    : s
                )
              );
            },
            onToolComplete: (tool, content) => {
              const ts = new Date().toISOString();
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sessionId
                    ? {
                        ...s,
                        messages: s.messages.map((m) =>
                          m.id === streamingId
                            ? {
                                ...m,
                                tool_calls: (m.tool_calls ?? []).map((tc) =>
                                  tc.name === tool ? { ...tc, status: "done", output: content || tc.output } : tc
                                ),
                                execution_timeline: [
                                  ...(m.execution_timeline ?? []),
                                  {
                                    type: "tool_complete",
                                    tool,
                                    content: content || undefined,
                                    created_at: ts,
                                  },
                                ],
                              }
                            : m
                        ),
                      }
                    : s
                )
              );
            },
            onStatus: (content, kind) => {
              if (!content) return;
              const ts = new Date().toISOString();
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sessionId
                    ? {
                        ...s,
                        messages: s.messages.map((m) =>
                          m.id === streamingId
                            ? {
                                ...m,
                                tool_calls: [
                                  ...(m.tool_calls ?? []),
                                  { name: "status", status: "running", output: content },
                                ],
                                execution_timeline: [
                                  ...(m.execution_timeline ?? []),
                                  {
                                    type: "status",
                                    tool: kind || "status",
                                    content,
                                    created_at: ts,
                                  },
                                ],
                              }
                            : m
                        ),
                      }
                    : s
                )
              );
            },
            }
          );

          setSessions((prev) =>
            prev.map((s) =>
              s.id === sessionId
                ? {
                    ...s,
                    messages: s.messages.map((m) =>
                      m.id === streamingId
                        ? {
                            ...m,
                            model: "cloud-agent",
                            execution_backend: "sandbox",
                          }
                        : m
                    ),
                    updated_at: new Date().toISOString(),
                  }
                : s
            )
          );
          resetStream();
          return;
        }

        const reply = await sendStreaming(sessionId, content);
        const assistantMsg: ChatMessage = {
          id: `reply-${Date.now()}`,
          role: "assistant",
          content: reply,
          timestamp: new Date().toISOString(),
          model: "remote",
          execution_backend: "local",
        };
        setSessions((prev) =>
          prev.map((s) =>
            s.id === sessionId
              ? { ...s, messages: [...s.messages, assistantMsg], updated_at: new Date().toISOString() }
              : s
          )
        );
        resetStream();
      } catch (e) {
        const errMsg: ChatMessage = {
          id: `err-${Date.now()}`,
          role: "assistant",
          content: `⚠️ 连接失败: ${e}`,
          timestamp: new Date().toISOString(),
          execution_backend: "local",
        };
        setSessions((prev) =>
          prev.map((s) =>
            s.id === sessionId
              ? { ...s, messages: [...s.messages, errMsg], updated_at: new Date().toISOString() }
              : s
          )
        );
        resetStream();
      }
    },
    [
      activeSessionId,
      cloudAgentBySession,
      enforceSandboxExecution,
      extractGithubRepoUrl,
      projects,
      resetStream,
      sendStreaming,
      sessions,
      shouldUseSandboxAutoRoute,
      CLOUD_AGENT_MODEL,
      CLOUD_AGENT_EXECUTION_PROFILE,
    ]
  );

  const handleAddProject = useCallback(async (name: string, path: string) => {
    const project = await api.addProject(name, path);
    setProjects((prev) => [...prev, project]);
  }, []);

  const handleRemoveProject = useCallback(async (id: string) => {
    await api.removeProject(id);
    setProjects((prev) => prev.filter((p) => p.id !== id));
  }, []);

  return (
    <div className="flex flex-col h-screen bg-bg-primary text-text-primary rounded-2xl overflow-hidden border border-[#273042]">
      <TitleBar onToggleSidebar={() => setSidebarOpen(!sidebarOpen)} />
      <div className="flex flex-1 overflow-hidden relative bg-[#23262a]">
        {sidebarOpen && (
          <div className="md:hidden fixed inset-0 bg-black/50 z-10" onClick={() => setSidebarOpen(false)} />
        )}
        <div
          className={`${sidebarOpen ? "translate-x-0" : "-translate-x-full"} md:translate-x-0 transition-transform duration-200 absolute md:relative z-20 h-full`}
          style={{ width: sidebarWidth }}
        >
          <Sidebar
            page={page}
            onPageChange={setPage}
            sessions={sessions}
            activeSessionId={activeSessionId}
            projects={projects}
            onNewChat={handleNewChat}
            onSelectSession={handleSelectSession}
            onDeleteSession={handleDeleteSession}
            onAddProject={handleAddProject}
            onRemoveProject={handleRemoveProject}
          />
          <div
            className="hidden md:block absolute top-0 right-0 h-full w-1 cursor-col-resize bg-transparent hover:bg-white/10 active:bg-white/20"
            onMouseDown={() => {
              resizingSidebarRef.current = true;
              document.body.style.cursor = "col-resize";
              document.body.style.userSelect = "none";
            }}
          />
        </div>
        <main className="flex-1 flex flex-col overflow-hidden rounded-[25px] bg-bg-primary">
          {page === "chat" && (
            <div className="flex flex-1 overflow-hidden">
              <ChatView
                session={activeSession}
                projects={projects}
                onSendMessage={handleSendMessage}
                onNewChat={handleNewChat}
                streamingText={streamingContent}
                isStreaming={isStreaming}
                environmentLabel={activeSessionCloudAgentId ? "Sandbox (Cloud Agent)" : "Local OpenAI"}
                isSandbox={Boolean(activeSessionCloudAgentId)}
                branch="main"
              />
              {activeSessionCloudAgentId && (
                <RightPanel
                  agentId={activeSessionCloudAgentId}
                  branch="main"
                  onOpenComputerUse={() => {}}
                  onOpenGithub={() => {}}
                  onOpenWebSearch={() => {}}
                />
              )}
            </div>
          )}
          {page === "search" && <SearchView sessions={sessions} onSelectSession={handleSelectSession} />}
          {page === "plugins" && <PluginsView />}
          {page === "automation" && <AutomationView />}
          {page === "settings" && <SettingsView />}
        </main>
      </div>
    </div>
  );
}
