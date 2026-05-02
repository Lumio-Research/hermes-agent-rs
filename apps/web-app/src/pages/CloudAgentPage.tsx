import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Code2,
  Film,
  GitCommitHorizontal,
  Loader2,
  MessageSquarePlus,
  RefreshCcw,
  Send,
  Sparkles,
  Terminal,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import {
  createCloudAgent,
  deleteCloudAgent,
  getCloudAgentCommits,
  getCloudAgentMessages,
  getCloudAgentStatus,
  getCloudAgents,
  sendCloudAgentMessageStream,
  type CloudAgentCommitRecord,
  type CloudAgentMessageRecord,
  type CloudAgentSession,
} from "../api";

// ── Helpers ───────────────────────────────────────────────────────────────────

function relativeTime(ts: string): string {
  const n = Date.parse(ts);
  if (Number.isNaN(n)) return ts;
  const sec = Math.max(1, Math.floor((Date.now() - n) / 1000));
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour}h`;
  return `${Math.floor(hour / 24)}d`;
}

function shortName(url: string): string {
  return url.replace(/\.git$/, "").split("/").filter(Boolean).pop() || url;
}

function statusDot(status: string): string {
  if (status === "running" || status === "ready") return "bg-emerald-400";
  if (status === "creating" || status === "sleeping") return "bg-amber-400";
  return "bg-gray-500";
}

function taskTitle(session: CloudAgentSession): string {
  // Use first message as title, or fallback to repo name
  return shortName(session.repo_url);
}

// ── Templates ─────────────────────────────────────────────────────────────────

const TEMPLATES = [
  {
    id: "code",
    icon: Code2,
    title: "应用开发",
    desc: "编写代码、调试 Bug、优化性能",
    prompt: "帮我编写代码、调试 Bug、优化性能，交付生产级代码产物。",
    color: "text-emerald-400",
    bg: "bg-emerald-500/10 border-emerald-500/20",
  },
  {
    id: "video",
    icon: Film,
    title: "视频制作",
    desc: "AI 脚本、剪辑、字幕生成",
    prompt: "帮我制作视频，包括脚本编写、素材剪辑和字幕生成。",
    color: "text-violet-400",
    bg: "bg-violet-500/10 border-violet-500/20",
  },
  {
    id: "tool",
    icon: Wrench,
    title: "工具脚本",
    desc: "自动化脚本、数据处理",
    prompt: "帮我编写自动化脚本工具，采集并处理数据。",
    color: "text-amber-400",
    bg: "bg-amber-500/10 border-amber-500/20",
  },
  {
    id: "analyze",
    icon: Terminal,
    title: "项目理解",
    desc: "分析项目仓库、生成文档",
    prompt: "分析这个项目的代码结构，生成一份 Code Wiki 文档。",
    color: "text-blue-400",
    bg: "bg-blue-500/10 border-blue-500/20",
  },
];

// ── Component ─────────────────────────────────────────────────────────────────

export default function CloudAgentPage() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  const [sessions, setSessions] = useState<CloudAgentSession[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [messages, setMessages] = useState<CloudAgentMessageRecord[]>([]);
  const [commits, setCommits] = useState<CloudAgentCommitRecord[]>([]);
  const [inFlight, setInFlight] = useState(false);

  const [input, setInput] = useState("");
  const [showDetail, setShowDetail] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const selected = useMemo(
    () => sessions.find((s) => s.id === selectedId) ?? null,
    [sessions, selectedId],
  );

  // Group sessions by repo
  const grouped = useMemo(() => {
    const map = new Map<string, CloudAgentSession[]>();
    for (const s of sessions) {
      const key = shortName(s.repo_url);
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(s);
    }
    return map;
  }, [sessions]);

  // ── Data ────────────────────────────────────────────────────────────────

  const refreshSessions = useCallback(async () => {
    const resp = await getCloudAgents();
    setSessions(resp.sessions);
  }, []);

  const refreshSelected = useCallback(async (id: string) => {
    const [statusResp, msgResp, commitResp] = await Promise.all([
      getCloudAgentStatus(id),
      getCloudAgentMessages(id),
      getCloudAgentCommits(id),
    ]);
    setInFlight(statusResp.in_flight);
    setMessages(msgResp.messages);
    setCommits(commitResp.commits);
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, ...statusResp.session } : s)));
  }, []);

  useEffect(() => {
    setLoading(true);
    refreshSessions()
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [refreshSessions]);

  useEffect(() => {
    if (!selectedId) return;
    refreshSelected(selectedId).catch((e) => setError(String(e)));
  }, [selectedId, refreshSelected]);

  // Poll
  useEffect(() => {
    if (!selectedId) return;
    const t = setInterval(() => {
      getCloudAgentStatus(selectedId)
        .then((r) => {
          setInFlight(r.in_flight);
          setSessions((prev) => prev.map((s) => (s.id === selectedId ? { ...s, ...r.session } : s)));
        })
        .catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, [selectedId]);

  // Auto-scroll
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // ── Actions ─────────────────────────────────────────────────────────────

  /** Send a message. If no session is selected, create one first. */
  const handleSend = useCallback(
    async (text?: string) => {
      const msg = (text ?? input).trim();
      if (!msg) return;
      setSending(true);
      setError(null);
      setInput("");

      try {
        let sid = selectedId;

        // Auto-create agent if none selected
        if (!sid) {
          const created = await createCloudAgent({
            workspace_mode: "blank",
            mode: "on_demand",
          });
          await refreshSessions();
          sid = created.id;
          setSelectedId(sid);
        }

        // Optimistic user message
        setMessages((prev) => [
          ...prev,
          {
            id: `local-${Date.now()}`,
            session_id: sid!,
            role: "user",
            content: msg,
            status: "done",
            created_at: new Date().toISOString(),
          },
        ]);

        const replyId = `reply-${Date.now()}`;
        setMessages((prev) => [
          ...prev,
          {
            id: replyId,
            session_id: sid!,
            role: "assistant",
            content: "",
            status: "running",
            created_at: new Date().toISOString(),
          },
        ]);

        await sendCloudAgentMessageStream(sid!, { text: msg }, {
          onChunk: (piece) => {
            setMessages((prev) =>
              prev.map((m) => (m.id === replyId ? { ...m, content: `${m.content}${piece}` } : m))
            );
          },
          onDone: () => {
            setMessages((prev) =>
              prev.map((m) => (m.id === replyId ? { ...m, status: "done" } : m))
            );
          },
          onError: (err) => {
            setMessages((prev) =>
              prev.map((m) => (m.id === replyId ? { ...m, status: "error", content: err } : m))
            );
          },
        });
      } catch (e) {
        setError(String(e));
      } finally {
        setSending(false);
      }
    },
    [input, selectedId, refreshSessions],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await deleteCloudAgent(id);
        setSessions((prev) => prev.filter((s) => s.id !== id));
        if (selectedId === id) {
          setSelectedId(null);
          setMessages([]);
          setCommits([]);
        }
      } catch (e) {
        setError(String(e));
      }
    },
    [selectedId],
  );

  const handleNewTask = useCallback(() => {
    setSelectedId(null);
    setMessages([]);
    setCommits([]);
    setInput("");
    setTimeout(() => inputRef.current?.focus(), 50);
  }, []);

  // ── Render ──────────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="h-screen flex items-center justify-center bg-[#0a0e14] text-gray-400">
        <Loader2 className="h-5 w-5 animate-spin mr-2" />
        Loading...
      </div>
    );
  }

  return (
    <div className="h-screen flex bg-[#0a0e14] text-gray-200 overflow-hidden">
      {/* ── Sidebar ──────────────────────────────────────────────────────── */}
      <aside className="w-60 flex-shrink-0 border-r border-white/[0.06] flex flex-col bg-[#0d1117]">
        <div className="p-3">
          <button
            onClick={handleNewTask}
            className="w-full flex items-center gap-2 rounded-lg border border-white/[0.08] px-3 py-2 text-sm text-gray-300 hover:bg-white/[0.04] transition-colors"
          >
            <MessageSquarePlus className="h-4 w-4 text-gray-500" />
            新任务
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-2 pb-3">
          {[...grouped.entries()].map(([group, items]) => (
            <div key={group} className="mb-3">
              <div className="flex items-center gap-1.5 px-2 py-1.5 text-[11px] font-medium text-gray-500 uppercase tracking-wider">
                <span>📂</span>
                <span className="truncate">{group}</span>
              </div>
              {items.map((s) => (
                <button
                  key={s.id}
                  onClick={() => setSelectedId(s.id)}
                  className={`group w-full flex items-center gap-2 rounded-md px-2.5 py-1.5 text-left transition-colors ${
                    selectedId === s.id
                      ? "bg-white/[0.08] text-white"
                      : "text-gray-400 hover:bg-white/[0.04] hover:text-gray-200"
                  }`}
                >
                  <span className={`h-1.5 w-1.5 rounded-full flex-shrink-0 ${statusDot(s.status)}`} />
                  <span className="flex-1 text-sm truncate">{taskTitle(s)}</span>
                  <span className="text-[10px] text-gray-600 flex-shrink-0">{relativeTime(s.last_active_at)}</span>
                  <span
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDelete(s.id).catch(() => {});
                    }}
                    className="hidden group-hover:inline-flex p-0.5 rounded hover:bg-white/10 text-gray-600 hover:text-red-400"
                  >
                    <X className="h-3 w-3" />
                  </span>
                </button>
              ))}
            </div>
          ))}
          {sessions.length === 0 && (
            <div className="text-xs text-gray-600 text-center py-10">
              还没有任务
            </div>
          )}
        </div>
      </aside>

      {/* ── Main ─────────────────────────────────────────────────────────── */}
      <main className="flex-1 flex flex-col min-w-0">
        {selected ? (
          <>
            {/* Top bar */}
            <div className="flex items-center justify-between px-4 py-2 border-b border-white/[0.06]">
              <div className="flex items-center gap-2 min-w-0">
                <span className={`h-2 w-2 rounded-full ${statusDot(selected.status)}`} />
                <span className="text-sm font-medium truncate">{taskTitle(selected)}</span>
                <span className="text-xs text-gray-600">{selected.branch}</span>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => refreshSelected(selected.id).catch(() => {})}
                  className="p-1.5 rounded hover:bg-white/[0.06] text-gray-500"
                >
                  <RefreshCcw className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => setShowDetail(!showDetail)}
                  className={`p-1.5 rounded hover:bg-white/[0.06] ${showDetail ? "text-blue-400" : "text-gray-500"}`}
                >
                  <GitCommitHorizontal className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => handleDelete(selected.id).catch(() => {})}
                  className="p-1.5 rounded hover:bg-white/[0.06] text-gray-500 hover:text-red-400"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>

            {/* Chat + optional detail panel */}
            <div className="flex-1 flex overflow-hidden">
              {/* Messages */}
              <div className="flex-1 flex flex-col min-w-0">
                <div className="flex-1 overflow-y-auto px-4 py-4 space-y-3">
                  {messages.length === 0 && (
                    <div className="flex items-center justify-center h-full">
                      <p className="text-sm text-gray-600">发送消息开始任务</p>
                    </div>
                  )}
                  {messages.map((msg) => (
                    <div key={msg.id} className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
                      <div
                        className={`max-w-[75%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed ${
                          msg.role === "user"
                            ? "bg-blue-600 text-white"
                            : "bg-white/[0.05] text-gray-200 border border-white/[0.06]"
                        }`}
                      >
                        <div className="whitespace-pre-wrap">{msg.content}</div>
                      </div>
                    </div>
                  ))}
                  {inFlight && (
                    <div className="flex justify-start">
                      <div className="bg-white/[0.05] border border-white/[0.06] rounded-2xl px-4 py-3">
                        <div className="flex items-center gap-1.5">
                          <span className="h-1.5 w-1.5 rounded-full bg-blue-400 animate-pulse" />
                          <span className="h-1.5 w-1.5 rounded-full bg-blue-400 animate-pulse [animation-delay:0.2s]" />
                          <span className="h-1.5 w-1.5 rounded-full bg-blue-400 animate-pulse [animation-delay:0.4s]" />
                        </div>
                      </div>
                    </div>
                  )}
                  <div ref={messagesEndRef} />
                </div>

                {/* Input */}
                <div className="px-4 py-3 border-t border-white/[0.06]">
                  {error && (
                    <div className="mb-2 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-300 flex items-center justify-between">
                      <span className="truncate">{error}</span>
                      <button onClick={() => setError(null)} className="ml-2 text-red-400 hover:text-red-200 flex-shrink-0">
                        <X className="h-3 w-3" />
                      </button>
                    </div>
                  )}
                  <div className="flex gap-2">
                    <input
                      ref={inputRef}
                      value={input}
                      onChange={(e) => setInput(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && !e.shiftKey) {
                          e.preventDefault();
                          handleSend().catch(() => {});
                        }
                      }}
                      placeholder="描述你想做的事..."
                      className="flex-1 rounded-xl border border-white/[0.08] bg-white/[0.03] px-4 py-2.5 text-sm placeholder-gray-600 focus:border-blue-500/50 focus:outline-none transition-colors"
                      disabled={sending}
                    />
                    <button
                      onClick={() => handleSend().catch(() => {})}
                      disabled={sending || !input.trim()}
                      className="rounded-xl bg-blue-600 px-4 py-2.5 text-white hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                    >
                      {sending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                    </button>
                  </div>
                </div>
              </div>

              {/* Detail panel */}
              {showDetail && (
                <aside className="w-64 flex-shrink-0 border-l border-white/[0.06] overflow-y-auto bg-[#0d1117]">
                  <div className="p-4 space-y-4">
                    {/* Commits */}
                    <div>
                      <h3 className="text-[11px] font-semibold text-gray-500 uppercase tracking-wider mb-2">Commits</h3>
                      {commits.length === 0 && <p className="text-xs text-gray-600">No commits</p>}
                      {commits.map((c) => (
                        <div key={c.id} className="rounded-lg border border-white/[0.06] bg-white/[0.02] p-2.5 mb-2">
                          <div className="flex items-center gap-1.5 text-xs font-mono text-blue-300">
                            <GitCommitHorizontal className="h-3 w-3" />
                            {c.commit_sha.slice(0, 8)}
                          </div>
                          <p className="text-[11px] text-gray-400 mt-1">{c.commit_message}</p>
                        </div>
                      ))}
                    </div>
                    {/* Info */}
                    <div>
                      <h3 className="text-[11px] font-semibold text-gray-500 uppercase tracking-wider mb-2">Details</h3>
                      <dl className="space-y-2 text-xs">
                        <div>
                          <dt className="text-gray-600">Repo</dt>
                          <dd className="text-gray-300 truncate">{selected.repo_url}</dd>
                        </div>
                        <div>
                          <dt className="text-gray-600">Sandbox</dt>
                          <dd className="font-mono text-gray-400 truncate">{selected.sandbox_id}</dd>
                        </div>
                        <div>
                          <dt className="text-gray-600">Created</dt>
                          <dd className="text-gray-400">{new Date(selected.created_at).toLocaleString()}</dd>
                        </div>
                      </dl>
                    </div>
                  </div>
                </aside>
              )}
            </div>
          </>
        ) : (
          /* ── Landing: no task selected ──────────────────────────────── */
          <div className="flex-1 flex flex-col items-center justify-center px-6">
            <div className="w-full max-w-2xl space-y-8">
              {/* Hero */}
              <div className="text-center">
                <div className="inline-flex items-center gap-2 mb-3">
                  <Sparkles className="h-6 w-6 text-blue-400" />
                  <h1 className="text-2xl font-semibold">What can I help you build?</h1>
                </div>
                <p className="text-sm text-gray-500">描述你的任务，AI 会在云端独立完成</p>
              </div>

              {/* Input */}
              <div className="relative">
                <input
                  ref={inputRef}
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault();
                      handleSend().catch(() => {});
                    }
                  }}
                  placeholder="描述你想做的事..."
                  className="w-full rounded-2xl border border-white/[0.08] bg-white/[0.03] px-5 py-4 text-base placeholder-gray-600 focus:border-blue-500/50 focus:outline-none transition-colors pr-14"
                  disabled={sending}
                  autoFocus
                />
                <button
                  onClick={() => handleSend().catch(() => {})}
                  disabled={sending || !input.trim()}
                  className="absolute right-3 top-1/2 -translate-y-1/2 rounded-xl bg-blue-600 p-2.5 text-white hover:bg-blue-500 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                >
                  {sending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                </button>
              </div>

              {/* Templates */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                {TEMPLATES.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => {
                      setInput(t.prompt);
                      inputRef.current?.focus();
                    }}
                    className={`rounded-xl border p-3.5 text-left transition-all hover:scale-[1.02] ${t.bg}`}
                  >
                    <t.icon className={`h-5 w-5 mb-2 ${t.color}`} />
                    <div className="text-sm font-medium text-gray-200">{t.title}</div>
                    <div className="text-[11px] text-gray-500 mt-0.5">{t.desc}</div>
                  </button>
                ))}
              </div>

              {error && (
                <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-2 text-sm text-red-300 text-center">
                  {error}
                </div>
              )}
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
