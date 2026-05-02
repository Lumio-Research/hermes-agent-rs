import { useState, useMemo } from "react";
import { Search, MessageSquare, Clock } from "lucide-react";
import type { Session } from "../types";

interface SearchViewProps {
  sessions: Session[];
  onSelectSession: (id: string) => void;
}

export function SearchView({ sessions, onSelectSession }: SearchViewProps) {
  const [query, setQuery] = useState("");

  const results = useMemo(() => {
    if (!query.trim()) return [];
    const q = query.toLowerCase();
    return sessions.filter(
      (s) =>
        s.title.toLowerCase().includes(q) ||
        s.messages.some((m) => m.content.toLowerCase().includes(q))
    );
  }, [query, sessions]);

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6">
      <div className="max-w-2xl mx-auto">
        <h1 className="text-2xl font-semibold text-text-primary mb-4">搜索</h1>

        {/* Search input */}
        <div className="relative mb-6">
          <Search
            size={18}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted"
          />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索聊天记录..."
            autoFocus
            className="w-full bg-bg-tertiary border border-border-primary rounded-lg pl-10 pr-4 py-2.5 text-sm text-text-primary placeholder-text-muted outline-none focus:border-accent transition-colors"
          />
        </div>

        {/* Results */}
        {query.trim() && results.length === 0 && (
          <p className="text-sm text-text-muted text-center py-8">
            没有找到匹配的聊天记录
          </p>
        )}

        <div className="space-y-2">
          {results.map((session) => {
            const matchMsg = session.messages.find((m) =>
              m.content.toLowerCase().includes(query.toLowerCase())
            );
            return (
              <button
                key={session.id}
                onClick={() => onSelectSession(session.id)}
                className="w-full text-left p-4 rounded-xl bg-bg-card border border-border-primary hover:bg-bg-card-hover hover:border-border-secondary transition-colors"
              >
                <div className="flex items-center gap-2 mb-1">
                  <MessageSquare size={14} className="text-text-muted" />
                  <span className="text-sm font-medium text-text-primary">
                    {session.title}
                  </span>
                </div>
                {matchMsg && (
                  <p className="text-xs text-text-muted line-clamp-2 ml-5">
                    {matchMsg.content.slice(0, 120)}...
                  </p>
                )}
                <div className="flex items-center gap-1 mt-2 ml-5">
                  <Clock size={10} className="text-text-muted" />
                  <span className="text-xs text-text-muted">
                    {new Date(session.updated_at).toLocaleDateString("zh-CN")}
                  </span>
                  <span className="text-xs text-text-muted ml-2">
                    {session.messages.length} 条消息
                  </span>
                </div>
              </button>
            );
          })}
        </div>

        {!query.trim() && (
          <div className="text-center py-16">
            <Search size={48} className="mx-auto text-text-muted/30 mb-4" />
            <p className="text-sm text-text-muted">
              输入关键词搜索所有聊天记录
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
