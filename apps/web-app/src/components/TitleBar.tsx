import { Menu } from "lucide-react";

interface TitleBarProps {
  onToggleSidebar: () => void;
}

export function TitleBar({ onToggleSidebar }: TitleBarProps) {
  return (
    <>
      {/* Mobile only: keep a tiny toolbar for sidebar toggle */}
      <div className="md:hidden h-10 flex items-center px-2 bg-bg-primary border-b border-border-primary shrink-0 select-none">
        <button
          onClick={onToggleSidebar}
          className="p-1.5 rounded hover:bg-bg-hover text-text-muted hover:text-text-secondary transition-colors"
        >
          <Menu size={18} />
        </button>
      </div>

      {/* Desktop title spacer removed by request */}
    </>
  );
}
