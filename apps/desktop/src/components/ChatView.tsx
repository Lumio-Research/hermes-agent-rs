import { useState, useRef, useEffect } from "react";
import { Send, Paperclip, AtSign, Slash, DollarSign } from "lucide-react";
import { MessageBubble } from "./MessageBubble";
import type { Session } from "../types";

interface ChatViewProps {
  session: Session | null;
  onSendMessage: (content: string) => Promise<void>;
  onNewChat: () => void;
  streamingText: string;
  isStreaming: boolean;
}

export function ChatView({ session, onSendMessage, onNewChat, streamingText, isStreaming }: ChatViewProps) {
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

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

    if (!session) {
      onNewChat();
      return;
    }

    setSending(true);
    setInput("");
    try {
      await onSendMessage(trimmed);
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
        <h1 className="text-2xl font-medium text-text-primary mb-2">What should we work on?</h1>
        <p className="text-text-muted text-sm mb-8">开始一个新的对话，或从左侧选择已有聊天</p>
        <div className="w-full max-w-2xl">
          <div className="bg-bg-tertiary border border-border-primary rounded-xl p-3">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="向 Hermes 提问，@ 添加文件，/ 输入命令，$ 使用技能"
              className="w-full bg-transparent text-text-primary placeholder-text-muted text-sm resize-none outline-none min-h-[40px]"
              rows={1}
            />
            <div className="flex items-center justify-between mt-2">
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 px-2 py-1 rounded-md text-xs text-text-muted hover:bg-bg-hover transition-colors">
                  <Paperclip size={14} />
                </button>
                <button className="flex items-center gap-1 px-2 py-1 rounded-md text-xs text-accent hover:bg-bg-hover transition-colors">
                  ⊘ 完全访问权限
                </button>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-text-muted">Local OpenAI</span>
                <span className="text-xs text-text-muted">GPT-5.2</span>
                <button
                  onClick={() => { onNewChat(); handleSubmit(); }}
                  disabled={!input.trim()}
                  className="p-1.5 rounded-full bg-text-muted/20 hover:bg-text-muted/30 text-text-secondary disabled:opacity-30 transition-colors"
                >
                  <Send size={14} />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Active chat
  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        <div className="max-w-3xl mx-auto space-y-4">
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
      <div className="border-t border-border-primary px-6 py-3">
        <div className="max-w-3xl mx-auto">
          <div className="bg-bg-tertiary border border-border-primary rounded-xl p-3">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="输入消息..."
              disabled={isStreaming}
              className="w-full bg-transparent text-text-primary placeholder-text-muted text-sm resize-none outline-none min-h-[40px] disabled:opacity-50"
              rows={1}
            />
            <div className="flex items-center justify-between mt-2">
              <div className="flex items-center gap-1">
                <InputHint icon={<AtSign size={12} />} label="文件" />
                <InputHint icon={<Slash size={12} />} label="命令" />
                <InputHint icon={<DollarSign size={12} />} label="技能" />
              </div>
              <button
                onClick={handleSubmit}
                disabled={!input.trim() || sending || isStreaming}
                className="p-1.5 rounded-full bg-accent hover:bg-accent-hover text-white disabled:opacity-30 transition-colors"
              >
                <Send size={14} />
              </button>
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
