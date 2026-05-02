import { useState, useRef, useEffect } from "react";
import { Send, AtSign, FolderOpen, ChevronDown, Monitor } from "lucide-react";
import { MessageBubble } from "./MessageBubble";
import type { Session, Project } from "../types";

interface ChatViewProps {
  session: Session | null;
  projects: Project[];
  onSendMessage: (content: string, projectId?: string) => Promise<void>;
  onNewChat?: () => void;
  streamingText: string;
  isStreaming: boolean;
  environmentLabel?: string;
}

export function ChatView({ session, projects, onSendMessage, streamingText, isStreaming, environmentLabel }: ChatViewProps) {
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [showProjectPicker, setShowProjectPicker] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const selectedProject = projects.find((p) => p.id === selectedProjectId) ?? null;

  // Auto-scroll to bottom when messages change or streaming text updates
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session?.messages, streamingText]);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 200) + "px";
    }
  }, [input]);

  const handleSubmit = async () => {
    const trimmed = input.trim();
    if (!trimmed || sending) return;
    setSending(true);
    setInput("");
    try {
      await onSendMessage(trimmed, selectedProjectId || undefined);
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  // Empty state
  if (!session) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center px-8">
        <h1 className="text-4xl font-semibold text-text-primary mb-2">What should we work on?</h1>
        <p className="text-text-muted text-sm mb-8">选择工作目录，然后开始对话。</p>

        {/* Template cards */}
        <div className="w-full max-w-3xl grid grid-cols-2 gap-3 mb-6">
          {[
            { icon: "📱", label: "应用开发", desc: "开发一个新功能或修复 Bug" },
            { icon: "🔍", label: "项目理解", desc: "分析项目结构，生成文档" },
            { icon: "⚙️", label: "代码重构", desc: "优化性能、架构设计" },
            { icon: "🔧", label: "工具脚本", desc: "编写自动化脚本，采集数据" },
          ].map((t) => (
            <button
              key={t.label}
              onClick={() => setInput(t.desc)}
              className="flex items-start gap-3 p-4 rounded-xl border border-[#2a3345] bg-[#1a2030] hover:border-[#3a5070] hover:bg-[#1e2840] transition-all text-left"
            >
              <span className="text-2xl mt-0.5">{t.icon}</span>
              <div>
                <p className="text-[13px] font-semibold text-[#c3cddd]">{t.label}</p>
                <p className="text-[11px] text-[#6b7a94] mt-0.5">{t.desc}</p>
              </div>
            </button>
          ))}
        </div>

        {/* Input box */}
        <div className="w-full max-w-3xl">
          <div className="bg-[#1a2030] border border-[#2a3345] rounded-2xl shadow-[0_8px_24px_rgba(0,0,0,0.4)]">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="帮你编写代码、调试 Bug、优化性能等开发工作，交付生产级代码产物。"
              className="w-full bg-transparent text-text-primary placeholder-[#4b5a72] text-sm resize-none outline-none px-4 pt-4 pb-2 min-h-[52px]"
              rows={2}
            />

            {/* Bottom bar */}
            <div className="flex items-center justify-between px-3 pb-3 pt-1 gap-2">
              {/* Left: workspace + env selectors */}
              <div className="flex items-center gap-2">
                {/* Workspace picker */}
                <div className="relative">
                  <button
                    onClick={() => setShowProjectPicker((v) => !v)}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-[#2a3345] bg-[#111827] text-[12px] text-[#9ba8be] hover:border-[#3a5070] hover:text-[#c3cddd] transition-colors max-w-[180px]"
                  >
                    <FolderOpen size={13} className={selectedProject ? "text-[#6b9eff]" : ""} />
                    <span className="truncate">
                      {selectedProject ? selectedProject.name : "选择工作目录"}
                    </span>
                    <ChevronDown size={11} className="shrink-0" />
                  </button>

                  {showProjectPicker && (
                    <div className="absolute bottom-full mb-2 left-0 w-64 rounded-xl border border-[#2a3345] bg-[#111827] shadow-2xl overflow-hidden z-50">
                      <div className="px-3 py-2 border-b border-[#1e2a3d]">
                        <span className="text-[11px] text-[#4b5a72] uppercase tracking-wider">工作目录</span>
                      </div>
                      {projects.length === 0 && (
                        <div className="px-3 py-3 text-[12px] text-[#4b5a72]">
                          暂无项目，请先在侧边栏添加目录
                        </div>
                      )}
                      {projects.map((p) => (
                        <button
                          key={p.id}
                          onClick={() => { setSelectedProjectId(p.id); setShowProjectPicker(false); }}
                          className={`flex items-center gap-2 w-full px-3 py-2.5 text-left text-[12px] hover:bg-[#1e2a3d] transition-colors ${
                            selectedProjectId === p.id ? "text-[#6b9eff]" : "text-[#9ba8be]"
                          }`}
                        >
                          <FolderOpen size={13} className={selectedProjectId === p.id ? "text-[#6b9eff]" : "text-[#4b5a72]"} />
                          <div className="min-w-0">
                            <p className="font-medium truncate">{p.name}</p>
                            <p className="text-[10px] text-[#4b5a72] truncate">{p.path}</p>
                          </div>
                        </button>
                      ))}
                      <button
                        onClick={() => { setSelectedProjectId(""); setShowProjectPicker(false); }}
                        className="flex items-center gap-2 w-full px-3 py-2.5 text-left text-[12px] text-[#4b5a72] hover:bg-[#1e2a3d] border-t border-[#1e2a3d] transition-colors"
                      >
                        不绑定目录
                      </button>
                    </div>
                  )}
                </div>

                {/* Environment indicator */}
                <div className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-[#2a3345] bg-[#111827] text-[12px] text-[#6b7a94]">
                  <Monitor size={12} />
                  <span>本地</span>
                </div>
              </div>

              {/* Right: send */}
              <button
                onClick={handleSubmit}
                disabled={!input.trim() || sending}
                className="flex items-center justify-center w-8 h-8 rounded-lg bg-[#3a5a9a] hover:bg-[#4a6fa5] text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors shrink-0"
              >
                <Send size={14} />
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Active chat
  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-bg-primary">
      <div className="px-8 pt-5 pb-3">
        <div className="max-w-4xl mx-auto flex items-center justify-between gap-3 text-[13px] text-text-secondary">
          <div className="flex items-center gap-2">
            <span className="font-medium text-text-primary leading-6">{session.title || "New chat"}</span>
            <span className="text-text-muted leading-6">···</span>
          </div>
        </div>
      </div>
      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-8 py-3">
        <div className="max-w-4xl mx-auto space-y-5">
          {session.messages.map((msg) => (
            <MessageBubble key={msg.id} message={msg} />
          ))}

          {/* Streaming assistant message */}
          {isStreaming && streamingText && (
            <MessageBubble
              message={{
                id: "streaming",
                role: "assistant",
                content: streamingText,
                timestamp: new Date().toISOString(),
                model: undefined,
              }}
            />
          )}

          {/* Thinking indicator */}
          {isStreaming && !streamingText && (
            <div className="flex items-center gap-2 text-text-muted text-sm py-2">
              <div className="flex gap-1">
                <span className="w-1.5 h-1.5 bg-accent rounded-full animate-bounce" />
                <span className="w-1.5 h-1.5 bg-accent rounded-full animate-bounce [animation-delay:0.1s]" />
                <span className="w-1.5 h-1.5 bg-accent rounded-full animate-bounce [animation-delay:0.2s]" />
              </div>
              <span>Hermes is thinking...</span>
            </div>
          )}

          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Input */}
      <div className="px-8 pb-3">
        <div className="max-w-4xl mx-auto">
          <div className="bg-[#1d2533] border border-[#313a4b] rounded-2xl shadow-[0_14px_40px_rgba(0,0,0,0.4)]">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a message..."
              disabled={isStreaming}
              className="w-full bg-transparent text-text-primary placeholder-text-muted text-sm resize-none outline-none px-4 pt-3 pb-1 min-h-[34px] disabled:opacity-50"
              rows={1}
            />
            <div className="flex items-center justify-between px-3 pb-2.5 pt-1 gap-2">
              {/* Left: workspace selector */}
              <div className="relative flex items-center gap-2">
                <button
                  onClick={() => setShowProjectPicker((v) => !v)}
                  className="flex items-center gap-1.5 px-2 py-1 rounded-md border border-[#273041] bg-transparent text-[11px] text-[#8a9ab8] hover:border-[#3a4d6a] hover:text-[#c3cddd] transition-colors max-w-[160px]"
                >
                  <FolderOpen size={11} className={selectedProject ? "text-[#6b9eff]" : ""} />
                  <span className="truncate">{selectedProject ? selectedProject.name : "工作目录"}</span>
                  <ChevronDown size={10} className="shrink-0" />
                </button>

                {showProjectPicker && (
                  <div className="absolute bottom-full mb-2 left-0 w-60 rounded-xl border border-[#2a3345] bg-[#111827] shadow-2xl z-50 overflow-hidden">
                    <div className="px-3 py-2 border-b border-[#1e2a3d]">
                      <span className="text-[11px] text-[#4b5a72] uppercase tracking-wider">工作目录</span>
                    </div>
                    {projects.length === 0 && (
                      <div className="px-3 py-3 text-[12px] text-[#4b5a72]">侧边栏添加项目后显示</div>
                    )}
                    {projects.map((p) => (
                      <button
                        key={p.id}
                        onClick={() => { setSelectedProjectId(p.id); setShowProjectPicker(false); }}
                        className={`flex items-center gap-2 w-full px-3 py-2.5 text-left text-[12px] hover:bg-[#1e2a3d] transition-colors ${selectedProjectId === p.id ? "text-[#6b9eff]" : "text-[#9ba8be]"}`}
                      >
                        <FolderOpen size={12} />
                        <div className="min-w-0">
                          <p className="font-medium truncate">{p.name}</p>
                          <p className="text-[10px] text-[#4b5a72] truncate">{p.path}</p>
                        </div>
                      </button>
                    ))}
                    <button
                      onClick={() => { setSelectedProjectId(""); setShowProjectPicker(false); }}
                      className="flex items-center gap-2 w-full px-3 py-2.5 text-[12px] text-[#4b5a72] hover:bg-[#1e2a3d] border-t border-[#1e2a3d] transition-colors"
                    >
                      不绑定目录
                    </button>
                  </div>
                )}

                <InputHint icon={<AtSign size={12} />} label="Default permissions" />
              </div>

              {/* Right: send */}
              <div className="flex items-center gap-2">
                <span className="px-2 py-0.5 rounded-md bg-[#273041] text-[11px] text-[#ced8ea]">
                  {environmentLabel ?? "Local OpenAI"}
                </span>
                <button
                  onClick={handleSubmit}
                  disabled={!input.trim() || sending || isStreaming}
                  className="p-1.5 rounded-full bg-text-muted/20 hover:bg-text-muted/30 text-text-secondary disabled:opacity-30 transition-colors"
                >
                  <Send size={14} />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function InputHint({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <span className="flex items-center gap-1 px-1.5 py-0.5 rounded text-xs text-text-muted">
      {icon}
      {label}
    </span>
  );
}

