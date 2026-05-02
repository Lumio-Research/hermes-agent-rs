import {
  MessageSquarePlus,
  Search,
  Settings,
  FolderOpen,
  Trash2,
  Filter,
  PenSquare,
  FolderPlus,
  X,
  LogOut,
  User,
  FolderSearch,
} from "lucide-react";
import { clsx } from "clsx";
import { useState, useRef, useEffect } from "react";
import { BackendStatus } from "./BackendStatus";
import { pickDirectory } from "../desktopBridge";
import { useAuth } from "../contexts/AuthContext";
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
  { id: "search", icon: Search, label: "Search" },
];

function AddProjectModal({
  onConfirm,
  onClose,
}: {
  onConfirm: (name: string, path: string) => void;
  onClose: () => void;
}) {
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [nameTouched, setNameTouched] = useState(false);
  const pathRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    pathRef.current?.focus();
  }, []);

  const derivedName = path.split("/").filter(Boolean).pop() ?? "";

  const displayName = nameTouched ? name : derivedName;

  const handlePathChange = (v: string) => {
    setPath(v);
    if (!nameTouched) setName("");
  };

  const handlePickDir = async () => {
    const selected = await pickDirectory();
    if (selected) {
      setPath(selected);
      if (!nameTouched) setName("");
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimPath = path.trim();
    if (!trimPath) return;
    const finalName = (nameTouched ? name : derivedName).trim() || trimPath;
    onConfirm(finalName, trimPath);
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      onKeyDown={handleKeyDown}
    >
      <div className="w-full max-w-md rounded-2xl border border-[#2e3a50] bg-[#1a2030] shadow-2xl p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <div className="p-1.5 rounded-lg bg-[#273041]">
              <FolderSearch size={16} className="text-[#6b9eff]" />
            </div>
            <span className="text-[15px] font-semibold text-[#e8eef8]">Add Project</span>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-md hover:bg-[#2b3444] text-[#9ba8be] hover:text-[#e8eef8] transition-colors"
          >
            <X size={14} />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-3">
          {/* Path row */}
          <div>
            <label className="block text-[11px] text-[#8f9ab0] uppercase tracking-wider mb-1.5">
              Project Path
            </label>
            <div className="flex gap-2">
              <input
                ref={pathRef}
                value={path}
                onChange={(e) => handlePathChange(e.target.value)}
                placeholder="/Users/me/my-project"
                className="flex-1 rounded-lg border border-[#2e3a50] bg-[#111827] px-3 py-2 text-sm text-[#e8eef8] placeholder-[#4b5a72] outline-none focus:border-[#4a6fa5] focus:ring-1 focus:ring-[#4a6fa5]/30 transition"
              />
              <button
                type="button"
                onClick={handlePickDir}
                title="Browse directory"
                className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-[#2e3a50] bg-[#1e2a3d] text-[#9ba8be] hover:bg-[#273041] hover:text-[#6b9eff] hover:border-[#4a6fa5] transition-colors text-xs whitespace-nowrap"
              >
                <FolderOpen size={13} />
                Browse
              </button>
            </div>
          </div>

          {/* Name row */}
          <div>
            <label className="block text-[11px] text-[#8f9ab0] uppercase tracking-wider mb-1.5">
              Display Name <span className="text-[#4b5a72] normal-case">(optional)</span>
            </label>
            <input
              value={displayName}
              onChange={(e) => { setName(e.target.value); setNameTouched(true); }}
              placeholder={derivedName || "my-project"}
              className="w-full rounded-lg border border-[#2e3a50] bg-[#111827] px-3 py-2 text-sm text-[#e8eef8] placeholder-[#4b5a72] outline-none focus:border-[#4a6fa5] focus:ring-1 focus:ring-[#4a6fa5]/30 transition"
            />
          </div>

          {/* Preview */}
          {path.trim() && (
            <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[#111827] border border-[#1e2a3d]">
              <FolderOpen size={13} className="text-[#6b9eff] shrink-0" />
              <div className="min-w-0">
                <p className="text-[12px] font-medium text-[#c3cddd] truncate">
                  {(nameTouched ? name : derivedName).trim() || derivedName || path.trim()}
                </p>
                <p className="text-[10px] text-[#4b5a72] truncate">{path.trim()}</p>
              </div>
            </div>
          )}

          {/* Actions */}
          <div className="flex gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 rounded-lg border border-[#2e3a50] px-3 py-2 text-sm text-[#9ba8be] hover:bg-[#1e2a3d] transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!path.trim()}
              className="flex-1 rounded-lg bg-[#3a5a9a] px-3 py-2 text-sm text-white font-medium hover:bg-[#4a6fa5] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              Add Project
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

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
  const { user, logout } = useAuth();
  const [showAddProject, setShowAddProject] = useState(false);
  const formatTime = (ts: string) => {
    const d = new Date(ts);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffH = Math.floor(diffMs / 3600000);
    const diffD = Math.floor(diffMs / 86400000);
    if (diffH < 1) return "now";
    if (diffH < 24) return `${diffH}h`;
    return `${diffD}d`;
  };

  const handleAddProject = () => setShowAddProject(true);

  return (
    <>
    {showAddProject && (
      <AddProjectModal
        onConfirm={(name, path) => onAddProject(name, path)}
        onClose={() => setShowAddProject(false)}
      />
    )}
    <aside className="relative w-full bg-[#23262a] flex h-full flex-col shrink-0 overflow-hidden">
      <div className="pointer-events-none absolute -left-20 bottom-10 h-72 w-72 rounded-full bg-[radial-gradient(circle,rgba(90,113,150,0.18)_0%,rgba(28,36,49,0.05)_52%,transparent_76%)]" />
      {/* New Chat */}
      <div className="px-2 py-2.5">
        <button
          onClick={onNewChat}
          className={clsx(
            "relative z-10 flex items-center gap-2 w-full px-2.5 py-2 rounded-md text-[13px] transition-colors",
            page === "chat" && !activeSessionId
              ? "bg-[#333c4d] text-[#e8eef8]"
              : "text-[#c3cddd] hover:bg-[#2b3444] hover:text-[#e8eef8]"
          )}
        >
          <MessageSquarePlus size={15} />
          <span>New chat</span>
        </button>
      </div>

      {/* Navigation */}
      <nav className="relative z-10 px-2 space-y-0.5">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            onClick={() => onPageChange(item.id)}
            className={clsx(
              "flex items-center gap-2 w-full px-2.5 py-1.5 rounded-md text-[13px] transition-colors",
              page === item.id
                ? "bg-[#333c4d] text-[#e8eef8]"
                : "text-[#c3cddd] hover:bg-[#2b3444] hover:text-[#e8eef8]"
            )}
          >
            <item.icon size={14} />
            <span>{item.label}</span>
          </button>
        ))}
      </nav>

      {/* Projects */}
      <div className="relative z-10 mt-4 px-2.5">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] text-[#8f9ab0] uppercase tracking-wider">Projects</span>
          <button
            onClick={handleAddProject}
            className="p-0.5 rounded hover:bg-[#2b3444] text-[#9ba8be] hover:text-[#d5deef]"
            title="Add project"
          >
            <FolderPlus size={12} />
          </button>
        </div>
        {projects.length === 0 && (
          <p className="text-xs text-[#9ba8be] px-3 py-1">No projects</p>
        )}
        {projects.map((p) => (
          <div
            key={p.id}
            className="group flex items-center gap-2 w-full px-2.5 py-1.5 rounded-md text-[12px] text-[#c3cddd] hover:bg-[#2b3444] hover:text-[#e8eef8] transition-colors"
          >
            <FolderOpen size={14} className="shrink-0" />
            <span className="truncate flex-1" title={p.path}>{p.name}</span>
            <button
              onClick={() => onRemoveProject(p.id)}
              className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[#333c4d] text-[#9ba8be] hover:text-error transition-all"
            >
              <X size={10} />
            </button>
          </div>
        ))}
      </div>

      {/* Chat History */}
      <div className="relative z-10 mt-4 px-2.5 flex-1 overflow-y-auto">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] text-[#8f9ab0] uppercase tracking-wider">Chats</span>
          <div className="flex items-center gap-1">
            <button className="p-0.5 rounded hover:bg-[#2b3444] text-[#9ba8be]">
              <Filter size={12} />
            </button>
            <button className="p-0.5 rounded hover:bg-[#2b3444] text-[#9ba8be]">
              <PenSquare size={12} />
            </button>
          </div>
        </div>
        <div className="space-y-0.5">
          {sessions.map((session) => (
            <div
              key={session.id}
              className={clsx(
                "group flex items-center justify-between px-2.5 py-1.5 rounded-md text-[12px] cursor-pointer transition-colors",
                activeSessionId === session.id && page === "chat"
                  ? "bg-[#333c4d] text-[#e8eef8]"
                  : "text-[#c3cddd] hover:bg-[#2b3444]"
              )}
              onClick={() => onSelectSession(session.id)}
            >
              <span className="truncate flex-1">{session.title}</span>
              <div className="flex items-center gap-2 shrink-0">
                <span className="text-[11px] text-[#97a3ba]">{formatTime(session.updated_at)}</span>
                <button
                  onClick={(e) => { e.stopPropagation(); onDeleteSession(session.id); }}
                  className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[#333c4d] text-[#9ba8be] hover:text-error transition-all"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Bottom: User + Status + Settings */}
      <div className="relative z-10 mt-auto p-2.5 space-y-1.5">
        {user && (
          <div className="flex items-center gap-2 px-2.5 py-2 rounded-md bg-[#1e2430] border border-[#2b3444]">
            <div className="flex items-center justify-center w-6 h-6 rounded-full bg-[#3a4766] shrink-0">
              <User size={12} className="text-[#a8b8d8]" />
            </div>
            <span className="flex-1 text-[11px] text-[#a8b8d8] truncate" title={user.email}>
              {user.email}
            </span>
            <button
              onClick={logout}
              title="Sign out"
              className="p-1 rounded hover:bg-[#2b3444] text-[#9ba8be] hover:text-red-400 transition-colors shrink-0"
            >
              <LogOut size={12} />
            </button>
          </div>
        )}
        <BackendStatus />
        <button
          onClick={() => onPageChange("settings")}
          className={clsx(
            "flex items-center gap-2.5 w-full px-2.5 py-2 rounded-md text-[13px] font-medium transition-colors justify-start",
            page === "settings"
              ? "bg-[#333c4d] text-[#e8eef8]"
              : "text-[#c3cddd] hover:bg-[#2b3444] hover:text-[#e8eef8]"
          )}
        >
          <Settings size={16} />
          <span>Settings</span>
        </button>
      </div>
    </aside>
    </>
  );
}
