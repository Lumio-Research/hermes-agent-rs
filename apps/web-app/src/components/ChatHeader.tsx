import { Settings2, Share2, MoreHorizontal } from "lucide-react";
import { formatRelativeTime } from "../utils/time";
import type { Session } from "../types";

interface ChatHeaderProps {
  session: Session;
  onOpenSettings?: () => void;
  onShare?: () => void;
}

/// Top bar inside ChatView. Shows session title, relative time, and a few
/// compact action icons. Layout-only — neutral icons from lucide-react.
export function ChatHeader({ session, onOpenSettings, onShare }: ChatHeaderProps) {
  const updated = formatRelativeTime(session.updated_at || session.created_at);

  return (
    <div className="px-8 pt-5 pb-3">
      <div className="max-w-4xl mx-auto flex items-center justify-between gap-3 text-[13px] text-text-secondary">
        <div className="flex items-center gap-2 min-w-0">
          <span className="font-medium text-text-primary leading-6 truncate">
            {session.title || "New chat"}
          </span>
          {updated && (
            <span className="text-text-muted leading-6 shrink-0">· {updated}</span>
          )}
        </div>
        <div className="flex items-center gap-1 text-text-muted shrink-0">
          {onShare && (
            <button
              onClick={onShare}
              title="Share"
              className="p-1.5 rounded-md hover:bg-white/[0.05] hover:text-text-primary transition-colors"
            >
              <Share2 size={14} />
            </button>
          )}
          {onOpenSettings && (
            <button
              onClick={onOpenSettings}
              title="Session settings"
              className="p-1.5 rounded-md hover:bg-white/[0.05] hover:text-text-primary transition-colors"
            >
              <Settings2 size={14} />
            </button>
          )}
          <button
            title="More"
            className="p-1.5 rounded-md hover:bg-white/[0.05] hover:text-text-primary transition-colors"
          >
            <MoreHorizontal size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}
