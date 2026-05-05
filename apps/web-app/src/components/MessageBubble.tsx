import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { Copy, Check } from "lucide-react";
import { useState } from "react";
import type { ChatMessage } from "../types";
import { ToolEventCard } from "./ToolEventCard";

interface MessageBubbleProps {
  message: ChatMessage;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === "user";

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div className={`${isUser ? "max-w-[42%]" : "max-w-[78%]"} min-w-0`}>
        <div
          className={`inline-block text-left text-[15px] leading-7 ${
            isUser
              ? "rounded-xl px-3.5 py-2 bg-[#273041] border border-[#374154] text-[#d7e1f4]"
              : "text-[#d0d8e8]"
          }`}
        >
          {isUser ? (
            <p className="whitespace-pre-wrap">{message.content}</p>
          ) : (
            <div className="prose prose-invert prose-sm max-w-none">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  code({ className, children, ...props }) {
                    const match = /language-(\w+)/.exec(className || "");
                    const codeString = String(children).replace(/\n$/, "");

                    if (match) {
                      return (
                        <CodeBlock language={match[1]} code={codeString} />
                      );
                    }
                    return (
                      <code
                        className="bg-bg-hover px-1.5 py-0.5 rounded text-accent text-xs"
                        {...props}
                      >
                        {children}
                      </code>
                    );
                  },
                }}
              >
                {message.content}
              </ReactMarkdown>
            </div>
          )}
        </div>

        {!isUser && message.execution_backend && (
          <div className="mt-1 text-[11px] text-text-muted">
            {message.execution_backend === "sandbox" ? "Sandbox backend" : "Local backend"}
          </div>
        )}

        {message.execution_timeline && message.execution_timeline.length > 0 && (
          <ToolEventCard events={message.execution_timeline} />
        )}

        {/* Tool calls (fallback) */}
        {(!message.execution_timeline || message.execution_timeline.length === 0) &&
          message.tool_calls &&
          message.tool_calls.length > 0 && (
          <div className="mt-2 space-y-1">
            {message.tool_calls.map((tc, i) => (
              <div
                key={i}
                className="flex items-center gap-2 text-xs text-text-muted"
              >
                <span
                  className={`w-1.5 h-1.5 rounded-full ${
                    tc.status === "done"
                      ? "bg-success"
                      : tc.status === "error"
                        ? "bg-error"
                        : "bg-warning animate-pulse"
                  }`}
                />
                <span className="font-mono">{tc.name}</span>
                <span>— {tc.status}</span>
                {tc.output ? <span className="truncate max-w-[420px]">{tc.output}</span> : null}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function CodeBlock({ language, code }: { language: string; code: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative group my-2 rounded-lg overflow-hidden">
      <div className="flex items-center justify-between px-3 py-1.5 bg-[#1e1e1e] text-xs text-text-muted">
        <span>{language}</span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 hover:text-text-secondary transition-colors"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          {copied ? "已复制" : "复制"}
        </button>
      </div>
      <SyntaxHighlighter
        language={language}
        style={oneDark}
        customStyle={{
          margin: 0,
          borderRadius: 0,
          fontSize: "12px",
          padding: "12px",
        }}
      >
        {code}
      </SyntaxHighlighter>
    </div>
  );
}
