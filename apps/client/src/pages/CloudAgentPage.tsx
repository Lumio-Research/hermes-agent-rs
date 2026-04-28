import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Bot,
  CheckCircle2,
  CirclePause,
  GitCommitHorizontal,
  Loader2,
  MonitorSmartphone,
  Plus,
  RefreshCcw,
  Rocket,
  Save,
  Send,
  Smartphone,
  Trash2,
} from "lucide-react";
import {
  createCloudAgent,
  deleteCloudAgent,
  getCloudAgentCommits,
  getCloudAgentMessages,
  getCloudAgentStatus,
  getCloudAgents,
  interruptCloudAgent,
  resolveWebSocketUrl,
  sendCloudAgentMessage,
  updateCloudAgentGitPolicy,
  type CloudAgentCommitRecord,
  type CloudAgentGitPolicy,
  type CloudAgentMessageRecord,
  type CloudAgentSession,
} from "../api";

const DEFAULT_GIT_POLICY: CloudAgentGitPolicy = {
  auto_commit_enabled: true,
  auto_push_enabled: false,
  target_branch: "main",
  protected_branches: ["main", "master"],
};

interface CloudAgentPageProps {
  embedded?: boolean;
}

function statusClass(status: string): string {
  if (status === "running" || status === "ready") return "text-emerald-300 border-emerald-500/30 bg-emerald-500/10";
  if (status === "creating" || status === "sleeping") return "text-amber-300 border-amber-500/30 bg-amber-500/10";
  if (status === "failed" || status === "destroyed" || status === "unreachable") return "text-red-300 border-red-500/30 bg-red-500/10";
  return "text-text-muted border-border-primary bg-bg-tertiary";
}

function shortRepoName(url: string): string {
  const n = url.replace(/\.git$/, "");
  return n.split("/").filter(Boolean).pop() || url;
}

function relativeTime(ts: string): string {
  const n = Date.parse(ts);
  if (Number.isNaN(n)) return ts;
  const sec = Math.max(1, Math.floor((Date.now() - n) / 1000));
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour}h ago`;
  return `${Math.floor(hour / 24)}d ago`;
}

export default function CloudAgentPage({ embedded = false }: CloudAgentPageProps) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [sending, setSending] = useState(false);
  const [savingPolicy, setSavingPolicy] = useState(false);
  const [inFlight, setInFlight] = useState(false);
  const [wsConnected, setWsConnected] = useState(false);

  const [sessions, setSessions] = useState<CloudAgentSession[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [messages, setMessages] = useState<CloudAgentMessageRecord[]>([]);
  const [commits, setCommits] = useState<CloudAgentCommitRecord[]>([]);
  const [logs, setLogs] = useState<Array<{ id: string; level: "info" | "error"; text: string }>>([]);

  const [repoUrl, setRepoUrl] = useState("");
  const [branch, setBranch] = useState("main");
  const [mode, setMode] = useState<"on_demand" | "persistent">("on_demand");
  const [model, setModel] = useState("");
  const [messageInput, setMessageInput] = useState("");
  const [gitPolicy, setGitPolicy] = useState<CloudAgentGitPolicy>(DEFAULT_GIT_POLICY);

  const wsRef = useRef<WebSocket | null>(null);

  const selected = useMemo(
    () => sessions.find((session) => session.id === selectedId) ?? null,
    [sessions, selectedId],
  );

  const pushLog = useCallback((level: "info" | "error", text: string) => {
    setLogs((prev) => [{ id: `${Date.now()}-${Math.random()}`, level, text }, ...prev].slice(0, 80));
  }, []);

  const refreshSessions = useCallback(async () => {
    const resp = await getCloudAgents();
    setSessions(resp.sessions);
    if (!selectedId && resp.sessions.length > 0) {
      setSelectedId(resp.sessions[0].id);
    }
  }, [selectedId]);

  const refreshSelected = useCallback(async (id: string) => {
    const [statusResp, messageResp, commitResp] = await Promise.all([
      getCloudAgentStatus(id),
      getCloudAgentMessages(id),
      getCloudAgentCommits(id),
    ]);
    setInFlight(statusResp.in_flight);
    setMessages(messageResp.messages);
    setCommits(commitResp.commits);
    setGitPolicy(statusResp.session.git_policy ?? DEFAULT_GIT_POLICY);
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, ...statusResp.session } : s)));
  }, []);

  useEffect(() => {
    setLoading(true);
    refreshSessions()
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [refreshSessions]);

  useEffect(() => {
    if (!selectedId) return;
    refreshSelected(selectedId).catch((err) => setError(String(err)));
  }, [selectedId, refreshSelected]);

  useEffect(() => {
    if (!selectedId) return;
    const timer = window.setInterval(() => {
      getCloudAgentStatus(selectedId)
        .then((statusResp) => {
          setInFlight(statusResp.in_flight);
          setSessions((prev) => prev.map((s) => (s.id === selectedId ? { ...s, ...statusResp.session } : s)));
        })
        .catch(() => {});
    }, 5000);
    return () => window.clearInterval(timer);
  }, [selectedId]);

  useEffect(() => {
    if (!selectedId) return;
    if (wsRef.current) wsRef.current.close();
    const ws = new WebSocket(resolveWebSocketUrl(`/api/v1/agents/${encodeURIComponent(selectedId)}/ws`));
    wsRef.current = ws;
    ws.onopen = () => {
      setWsConnected(true);
      pushLog("info", "WebSocket connected");
    };
    ws.onclose = () => {
      setWsConnected(false);
      pushLog("info", "WebSocket closed");
    };
    ws.onerror = () => {
      setWsConnected(false);
      pushLog("error", "WebSocket error");
    };
    ws.onmessage = (event) => {
      try {
        const payload = JSON.parse(String(event.data)) as { type?: string; content?: string };
        if (payload.type === "done") {
          const content = payload.content ?? "";
          setMessages((prev) => [
            ...prev,
            {
              id: `ws-assistant-${Date.now()}`,
              session_id: selectedId,
              role: "assistant",
              content,
              status: "done",
              created_at: new Date().toISOString(),
            },
          ]);
          getCloudAgentCommits(selectedId).then((resp) => setCommits(resp.commits)).catch(() => {});
        } else if (payload.type === "error" && payload.content) {
          setError(payload.content);
          pushLog("error", payload.content);
        }
      } catch {
        // ignore malformed payload
      }
    };
    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [selectedId, pushLog]);

  const handleCreate = useCallback(async () => {
    if (!repoUrl.trim()) return;
    setCreating(true);
    setError(null);
    try {
      const created = await createCloudAgent({
        repo_url: repoUrl.trim(),
        branch: branch.trim() || "main",
        mode,
        model: model.trim() || undefined,
      });
      await updateCloudAgentGitPolicy(created.id, gitPolicy).catch(() => {});
      await refreshSessions();
      setSelectedId(created.id);
      pushLog("info", `Agent created: ${created.id}`);
    } catch (err) {
      setError(String(err));
      pushLog("error", `Create failed: ${String(err)}`);
    } finally {
      setCreating(false);
    }
  }, [repoUrl, branch, mode, model, gitPolicy, refreshSessions, pushLog]);

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await deleteCloudAgent(id);
        setSessions((prev) => prev.filter((s) => s.id !== id));
        if (selectedId === id) setSelectedId(null);
      } catch (err) {
        setError(String(err));
      }
    },
    [selectedId],
  );

  const handleInterrupt = useCallback(async () => {
    if (!selectedId) return;
    try {
      await interruptCloudAgent(selectedId);
      await refreshSelected(selectedId);
    } catch (err) {
      setError(String(err));
    }
  }, [selectedId, refreshSelected]);

  const handleSend = useCallback(async () => {
    if (!selectedId || !messageInput.trim()) return;
    const input = messageInput.trim();
    setSending(true);
    setError(null);
    setMessages((prev) => [
      ...prev,
      {
        id: `local-user-${Date.now()}`,
        session_id: selectedId,
        role: "user",
        content: input,
        status: "done",
        created_at: new Date().toISOString(),
      },
    ]);
    try {
      if (wsRef.current && wsConnected) {
        wsRef.current.send(JSON.stringify({ text: input, model: model.trim() || undefined }));
      } else {
        const resp = await sendCloudAgentMessage(selectedId, { text: input, model: model.trim() || undefined });
        setMessages((prev) => [
          ...prev,
          {
            id: `local-assistant-${Date.now()}`,
            session_id: selectedId,
            role: "assistant",
            content: resp.reply ?? "",
            status: "done",
            created_at: new Date().toISOString(),
          },
        ]);
      }
      setMessageInput("");
    } catch (err) {
      setError(String(err));
    } finally {
      setSending(false);
    }
  }, [selectedId, messageInput, wsConnected, model]);

  const handleSavePolicy = useCallback(async () => {
    if (!selectedId) return;
    setSavingPolicy(true);
    try {
      const resp = await updateCloudAgentGitPolicy(selectedId, gitPolicy);
      setGitPolicy(resp.session.git_policy ?? gitPolicy);
      setSessions((prev) => prev.map((s) => (s.id === selectedId ? { ...s, ...resp.session } : s)));
      pushLog("info", "Git policy updated");
    } catch (err) {
      setError(String(err));
    } finally {
      setSavingPolicy(false);
    }
  }, [selectedId, gitPolicy, pushLog]);

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center text-text-muted">
        <div className="inline-flex items-center gap-2">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading cloud agent...
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto p-4 md:p-6 space-y-4">
      <section className="rounded-xl border border-border-primary bg-bg-card overflow-hidden">
        <div className="px-4 py-4 border-b border-border-primary bg-gradient-to-r from-[#1f2430] to-[#181c25]">
          <div className="inline-flex items-center gap-2 text-text-primary">
            <Bot className="h-4 w-4 text-accent" />
            <h1 className="text-xl font-semibold">Cloud Agent</h1>
          </div>
          <p className="text-xs text-text-muted mt-1">
            {embedded ? "Workspace integrated view" : "Unified cloud coding workspace"}
          </p>
        </div>
        <div className="p-4 space-y-2 text-sm">
          <div className="inline-flex items-center gap-2 text-text-secondary">
            <CheckCircle2 className="h-4 w-4 text-emerald-400" />
            连接云端仓库并在隔离环境内执行代码任务
          </div>
          <div className="inline-flex items-center gap-2 text-text-secondary">
            <CheckCircle2 className="h-4 w-4 text-emerald-400" />
            支持会话持续、自动提交策略与跨设备续接
          </div>
        </div>
      </section>

      <section className="grid gap-3 md:grid-cols-3">
        <div className="rounded-xl border border-border-primary bg-bg-card p-4 space-y-2">
          <div className="inline-flex items-center gap-2 text-sm text-text-primary">
            <Rocket className="h-4 w-4 text-accent" />
            在云端服务器上
          </div>
          <p className="text-xs text-text-muted">直接创建 Cloud Agent，并在浏览器内持续工作。</p>
          <button
            onClick={() => setMode("persistent")}
            className="rounded-md bg-accent px-3 py-1.5 text-sm text-white"
          >
            创建
          </button>
        </div>
        <div className="rounded-xl border border-border-primary bg-bg-card p-4 space-y-2">
          <div className="inline-flex items-center gap-2 text-sm text-text-primary">
            <MonitorSmartphone className="h-4 w-4 text-blue-300" />
            在我的电脑上
          </div>
          <p className="text-xs text-text-muted">下载桌面版体验本地能力（规划中）。</p>
          <button className="rounded-md border border-border-primary px-3 py-1.5 text-sm text-text-muted">
            即将上线
          </button>
        </div>
        <div className="rounded-xl border border-border-primary bg-bg-card p-4 space-y-2">
          <div className="inline-flex items-center gap-2 text-sm text-text-primary">
            <Smartphone className="h-4 w-4 text-violet-300" />
            在安卓/手机上
          </div>
          <p className="text-xs text-text-muted">移动端入口与扫码续接（规划中）。</p>
          <button className="rounded-md border border-border-primary px-3 py-1.5 text-sm text-text-muted">
            即将上线
          </button>
        </div>
      </section>

      <section className="rounded-xl border border-border-primary bg-bg-card p-4 space-y-3">
        <div className="text-sm font-medium text-text-primary">快速创建 Cloud Agent</div>
        <div className="grid gap-2 md:grid-cols-[1.6fr_0.8fr_0.8fr_1fr_auto]">
          <input
            value={repoUrl}
            onChange={(e) => setRepoUrl(e.target.value)}
            placeholder="https://github.com/org/repo"
            className="rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
          />
          <input
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
            placeholder="branch"
            className="rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
          />
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value as "on_demand" | "persistent")}
            className="rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
          >
            <option value="on_demand">on_demand</option>
            <option value="persistent">persistent</option>
          </select>
          <input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="model (optional)"
            className="rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
          />
          <button
            onClick={() => handleCreate().catch(() => {})}
            disabled={creating || !repoUrl.trim()}
            className="inline-flex items-center gap-1 rounded-md bg-accent px-3 py-2 text-sm text-white disabled:opacity-60"
          >
            {creating ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
            Create
          </button>
        </div>
      </section>

      <section className="rounded-xl border border-border-primary bg-bg-card p-4">
        <div className="text-sm font-medium text-text-primary mb-2">我的 Cloud Agent 会话</div>
        <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-3">
          {sessions.map((session) => (
            <button
              key={session.id}
              onClick={() => setSelectedId(session.id)}
              className={`rounded-lg border p-3 text-left transition-colors ${
                selectedId === session.id ? "border-accent/60 bg-accent/10" : "border-border-primary bg-bg-tertiary hover:bg-bg-hover"
              }`}
            >
              <div className="text-sm font-medium text-text-primary">{shortRepoName(session.repo_url)}</div>
              <div className="text-xs text-text-muted truncate mt-0.5">{session.branch} · {session.mode}</div>
              <div className="mt-2 flex items-center justify-between">
                <span className={`rounded border px-2 py-0.5 text-xs ${statusClass(session.status)}`}>{session.status}</span>
                <span className="text-[11px] text-text-muted">{relativeTime(session.last_active_at)}</span>
              </div>
              <div className="mt-2 text-right">
                <span
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDelete(session.id).catch(() => {});
                  }}
                  className="inline-flex items-center gap-1 text-xs text-red-300 hover:text-red-200"
                >
                  <Trash2 className="h-3 w-3" />
                  删除
                </span>
              </div>
            </button>
          ))}
          {sessions.length === 0 && (
            <div className="rounded-lg border border-dashed border-border-primary p-4 text-xs text-text-muted">
              还没有会话，先通过上方快速创建入口启动一个 Cloud Agent。
            </div>
          )}
        </div>
      </section>

      {selected && (
        <section className="grid gap-4 xl:grid-cols-[1fr_320px]">
          <div className="rounded-xl border border-border-primary bg-bg-card p-4 space-y-3">
            <div className="flex items-center justify-between border-b border-border-primary pb-2">
              <div className="text-sm text-text-secondary">
                {selected.repo_url} · {selected.branch}
              </div>
              <div className="inline-flex items-center gap-2">
                <span className={wsConnected ? "text-xs text-emerald-300" : "text-xs text-amber-300"}>
                  ws: {wsConnected ? "connected" : "disconnected"}
                </span>
                <button
                  onClick={() => selectedId && refreshSelected(selectedId).catch((err) => setError(String(err)))}
                  className="inline-flex items-center gap-1 rounded border border-border-primary px-2 py-1 text-xs hover:bg-bg-hover"
                >
                  <RefreshCcw className="h-3 w-3" />
                  刷新
                </button>
                <button
                  onClick={() => handleInterrupt().catch(() => {})}
                  disabled={!inFlight}
                  className="inline-flex items-center gap-1 rounded border border-border-primary px-2 py-1 text-xs hover:bg-bg-hover disabled:opacity-60"
                >
                  <CirclePause className="h-3 w-3" />
                  中断
                </button>
              </div>
            </div>

            <div className="h-[360px] overflow-y-auto rounded-md border border-border-primary bg-bg-tertiary p-2 space-y-2">
              {messages.map((msg) => (
                <div
                  key={msg.id}
                  className={`max-w-[88%] rounded-md border px-2.5 py-2 ${
                    msg.role === "user" ? "ml-auto border-accent/50 bg-accent/10" : "border-border-primary bg-bg-card"
                  }`}
                >
                  <div className="text-[10px] text-text-muted uppercase mb-1">{msg.role}</div>
                  <div className="text-sm whitespace-pre-wrap">{msg.content}</div>
                </div>
              ))}
              {messages.length === 0 && (
                <div className="rounded-md border border-dashed border-border-primary p-4 text-xs text-text-muted">
                  发送你的第一条指令，Cloud Agent 会在云端仓库执行并回复。
                </div>
              )}
            </div>

            <div className="flex gap-2">
              <input
                value={messageInput}
                onChange={(e) => setMessageInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    handleSend().catch(() => {});
                  }
                }}
                placeholder="Ask cloud agent..."
                className="w-full rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
                disabled={sending}
              />
              <button
                onClick={() => handleSend().catch(() => {})}
                disabled={sending || !messageInput.trim()}
                className="inline-flex items-center gap-1 rounded-md bg-accent px-3 py-2 text-sm text-white disabled:opacity-60"
              >
                {sending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                发送
              </button>
            </div>
          </div>

          <div className="rounded-xl border border-border-primary bg-bg-card p-4 space-y-3">
            <div className="text-sm font-semibold text-text-primary">Inspector</div>
            <div className="space-y-2 text-xs">
              <label className="flex items-center justify-between">
                Auto commit
                <input
                  type="checkbox"
                  checked={gitPolicy.auto_commit_enabled}
                  onChange={(e) => setGitPolicy((prev) => ({ ...prev, auto_commit_enabled: e.target.checked }))}
                />
              </label>
              <label className="flex items-center justify-between">
                Auto push
                <input
                  type="checkbox"
                  checked={gitPolicy.auto_push_enabled}
                  onChange={(e) => setGitPolicy((prev) => ({ ...prev, auto_push_enabled: e.target.checked }))}
                />
              </label>
              <input
                value={gitPolicy.target_branch}
                onChange={(e) => setGitPolicy((prev) => ({ ...prev, target_branch: e.target.value }))}
                className="w-full rounded border border-border-primary bg-bg-tertiary px-2 py-1"
                placeholder="target branch"
              />
              <input
                value={gitPolicy.protected_branches.join(",")}
                onChange={(e) =>
                  setGitPolicy((prev) => ({
                    ...prev,
                    protected_branches: e.target.value.split(",").map((v) => v.trim()).filter(Boolean),
                  }))
                }
                className="w-full rounded border border-border-primary bg-bg-tertiary px-2 py-1"
                placeholder="protected branches"
              />
              <button
                onClick={() => handleSavePolicy().catch(() => {})}
                disabled={savingPolicy}
                className="w-full inline-flex items-center justify-center gap-1 rounded border border-border-primary px-2 py-1.5 hover:bg-bg-hover disabled:opacity-60"
              >
                {savingPolicy ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
                保存策略
              </button>
            </div>

            <div className="border-t border-border-primary pt-2">
              <div className="text-xs text-text-muted mb-2">Commits</div>
              <div className="max-h-36 overflow-y-auto space-y-2">
                {commits.map((commit) => (
                  <div key={commit.id} className="rounded border border-border-primary bg-bg-tertiary p-2 text-xs">
                    <div className="inline-flex items-center gap-1 font-mono">
                      <GitCommitHorizontal className="h-3 w-3" />
                      {commit.commit_sha}
                    </div>
                    <div className="mt-1">{commit.commit_message}</div>
                  </div>
                ))}
                {commits.length === 0 && <div className="text-xs text-text-muted">暂无提交记录</div>}
              </div>
            </div>

            <div className="border-t border-border-primary pt-2">
              <div className="text-xs text-text-muted mb-2">Activity</div>
              <div className="max-h-28 overflow-y-auto space-y-1">
                {logs.map((log) => (
                  <div
                    key={log.id}
                    className={`rounded border px-2 py-1 text-xs ${
                      log.level === "error"
                        ? "border-red-500/40 bg-red-500/10 text-red-300"
                        : "border-border-primary bg-bg-tertiary text-text-secondary"
                    }`}
                  >
                    {log.text}
                  </div>
                ))}
                {logs.length === 0 && <div className="text-xs text-text-muted">暂无日志</div>}
              </div>
            </div>
          </div>
        </section>
      )}

      {error && <div className="text-xs text-red-400">{error}</div>}
    </div>
  );
}

