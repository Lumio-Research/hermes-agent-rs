import { ChevronLeft, ChevronRight, Menu } from "lucide-react";

interface TitleBarProps {
  onToggleSidebar: () => void;
}

export function TitleBar({ onToggleSidebar }: TitleBarProps) {
  return (
    <div
      data-tauri-drag-region
      className="h-12 flex items-center px-4 bg-bg-primary border-b border-border-primary shrink-0"
    >
      {/* macOS traffic lights space (desktop) / hamburger (mobile) */}
      <div className="w-20 hidden md:block" />
      <button
        onClick={onToggleSidebar}
        className="md:hidden p-1.5 rounded hover:bg-bg-hover text-text-muted hover:text-text-secondary transition-colors"
      >
        <Menu size={18} />
      </button>

      {/* Navigation buttons */}
      <div className="hidden md:flex items-center gap-1">
        <button className="p-1 rounded hover:bg-bg-hover text-text-muted hover:text-text-secondary transition-colors">
          <ChevronLeft size={16} />
        </button>
        <button className="p-1 rounded hover:bg-bg-hover text-text-muted hover:text-text-secondary transition-colors">
          <ChevronRight size={16} />
        </button>
      </div>

      <div className="flex-1" data-tauri-drag-region />

      <div className="flex items-center gap-3">
        <span className="text-xs text-text-muted font-medium pointer-events-none">Hermes Agent</span>
      </div>
    </div>
  );
}
