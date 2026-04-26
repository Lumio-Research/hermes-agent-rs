import {
  MessageSquarePlus,
  Search,
  Puzzle,
  Zap,
  Settings,
  FolderOpen,
  Trash2,
  Filter,
  PenSquare,
  FolderPlus,
  X,
} from "lucide-react";
import { clsx } from "clsx";
import { BackendStatus } from "./BackendStatus";
import type { Session, Project, NavPage } from "../types";

interface SidebarProps {
  page: NavPage;
  onPageChange: (page: NavPage) => void;
  sessions: Session[];
  activeSessionId: string | null;
  projects: Project[];
  onNewChat: () => void;
  onSelectSession: (id: string) => void;
  onDeleteSession: (id: string) => void;
  onAddProject: (name: string, path: string) => void;
  onRemoveProject: (id: string) => void;
}

const NAV_ITEMS: { id: NavPage; icon: typeof Search; label: string }[] = [
  { id: "search", icon: Search, label: "搜索" },
  { id: "plugins", icon: Puzzle, label: "插件" },
  { id: "automation", icon: Zap, label: "自动化" },
];

export function Sidebar({
  page,
  onPageChange,
  sessions,
  activeSessionId,
  projects,
  onNewChat,
  onSelectSession,
  onDeleteSession,
  onAddProject,
  onRemoveProject,
}: SidebarProps) {
  const formatTime = (ts: string) => {
    const d = new Date(ts);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffH = Math.floor(diffMs / 3600000);
    const diffD = Math.floor(diffMs / 86400000);
    if (diffH < 1) return "刚刚";
    if (diffH < 24) return `${diffH} 小时`;
    return `${diffD} 天`;
  };

  const handleAddProject = async () => {
    // Try Tauri dialog, fall back to prompt
    try {
      if ("__TAURI_INTERNALS__" in window) {
        const { open } = await import("@tauri-apps/plugin-dialog");
        const selected = await open({ directory: true, multiple: false, title: "选择项目目录" });
        if (selected) {
          const path = typeof selected === "string" ? selected : selected[0];
          if (path) {
            const name = path.split("/").pop() || path;
            onAddProject(name, path);
          }
        }
        return;
      }
    } catch { /* fall through */ }
    const path = prompt("输入项目路径:");
    if (path) {
      const name = path.split("/").pop() || path;
      onAddProject(name, path);
    }
  };

  return (
    <aside className="w-56 bg-bg-secondary border-r border-border-primary flex flex-col shrink-0 overflow-hidden">
      {/* New Chat */}
      <div className="p-3">
        <button
          onClick={onNewChat}
          className={clsx(
            "flex items-center gap-2 w-full px-3 py-2 rounded-lg text-sm transition-colors",
            page === "chat" && !activeSessionId
              ? "bg-bg-active text-text-primary"
              : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
          )}
        >
          <MessageSquarePlus size={16} />
          <span>新建聊天</span>
        </button>
      </div>

      {/* Navigation */}
      <nav className="px-3 space-y-0.5">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            onClick={() => onPageChange(item.id)}
            className={clsx(
              "flex items-center gap-2 w-full px-3 py-2 rounded-lg text-sm transition-colors",
              page === item.id
                ? "bg-bg-active text-text-primary"
                : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            )}
          >
            <item.icon size={16} />
            <span>{item.label}</span>
          </button>
        ))}
      </nav>

      {/* Projects */}
      <div className="mt-4 px-3">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs text-text-muted uppercase tracking-wider">项目</span>
          <button
            onClick={handleAddProject}
            className="p-0.5 rounded hover:bg-bg-hover text-text-muted hover:text-text-secondary"
            title="添加项目"
          >
            <FolderPlus size={12} />
          </button>
        </div>
        {projects.length === 0 && (
          <p className="text-xs text-text-muted px-3 py-1">暂无项目</p>
        )}
        {projects.map((p) => (
          <div
            key={p.id}
            className="group flex items-center gap-2 w-full px-3 py-1.5 rounded-lg text-sm text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors"
          >
            <FolderOpen size={14} className="shrink-0" />
            <span className="truncate flex-1" title={p.path}>{p.name}</span>
            <button
              onClick={() => onRemoveProject(p.id)}
              className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-bg-hover text-text-muted hover:text-error transition-all"
            >
              <X size={10} />
            </button>
          </div>
        ))}
      </div>

      {/* Chat History */}
      <div className="mt-4 px-3 flex-1 overflow-y-auto">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs text-text-muted uppercase tracking-wider">聊天</span>
          <div className="flex items-center gap-1">
            <button className="p-0.5 rounded hover:bg-bg-hover text-text-muted">
              <Filter size={12} />
            </button>
            <button className="p-0.5 rounded hover:bg-bg-hover text-text-muted">
              <PenSquare size={12} />
            </button>
          </div>
        </div>
        <div className="space-y-0.5">
          {sessions.map((session) => (
            <div
              key={session.id}
              className={clsx(
                "group flex items-center justify-between px-3 py-1.5 rounded-lg text-sm cursor-pointer transition-colors",
                activeSessionId === session.id && page === "chat"
                  ? "bg-bg-active text-text-primary"
                  : "text-text-secondary hover:bg-bg-hover"
              )}
              onClick={() => onSelectSession(session.id)}
            >
              <span className="truncate flex-1">{session.title}</span>
              <div className="flex items-center gap-2 shrink-0">
                <span className="text-xs text-text-muted">{formatTime(session.updated_at)}</span>
                <button
                  onClick={(e) => { e.stopPropagation(); onDeleteSession(session.id); }}
                  className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-bg-hover text-text-muted hover:text-error transition-all"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Bottom: Status + Settings */}
      <div className="p-3 border-t border-border-primary space-y-1">
        <BackendStatus />
        <button
          onClick={() => onPageChange("settings")}
          className={clsx(
            "flex items-center gap-2 w-full px-3 py-2 rounded-lg text-sm transition-colors",
            page === "settings"
              ? "bg-bg-active text-text-primary"
              : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
          )}
        >
          <Settings size={16} />
          <span>设置</span>
        </button>
      </div>
    </aside>
  );
}
