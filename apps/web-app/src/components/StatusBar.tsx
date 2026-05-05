import { Cloud, Monitor, GitBranch, Cpu } from "lucide-react";

interface StatusBarProps {
  /// Backend label, e.g. "Sandbox (Cloud Agent)" or "Local OpenAI".
  environmentLabel?: string;
  /// True when the current session is sandbox-backed.
  isSandbox?: boolean;
  /// Branch name for the active project / cloud agent.
  branch?: string;
  /// Token usage percentage in [0, 100], displayed as "本地模式 X%".
  usagePct?: number;
  /// Optional small callout to the right of branch (e.g. provider name).
  rightSlot?: React.ReactNode;
}

/// Thin footer showing the current execution mode, working branch, and a
/// rough token-usage indicator. Layout-only.
export function StatusBar({
  environmentLabel,
  isSandbox,
  branch,
  usagePct,
  rightSlot,
}: StatusBarProps) {
  return (
    <div className="px-8 pb-2 pt-1 border-t border-white/[0.04] bg-bg-primary">
      <div className="max-w-5xl mx-auto flex items-center justify-between gap-3 text-[11px] text-text-muted">
        <div className="flex items-center gap-3 min-w-0">
          <span
            className={`flex items-center gap-1 px-1.5 py-0.5 rounded-md border ${
              isSandbox
                ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-200"
                : "border-white/10 bg-white/5 text-text-secondary"
            }`}
          >
            {isSandbox ? <Cloud size={11} /> : <Monitor size={11} />}
            {environmentLabel ?? (isSandbox ? "Sandbox" : "本地模式")}
          </span>
          {typeof usagePct === "number" && Number.isFinite(usagePct) && (
            <span className="flex items-center gap-1">
              <Cpu size={11} />
              {Math.round(usagePct)}%
            </span>
          )}
          {branch && (
            <span className="flex items-center gap-1 truncate">
              <GitBranch size={11} />
              <span className="truncate">{branch}</span>
            </span>
          )}
        </div>
        {rightSlot && <div className="shrink-0">{rightSlot}</div>}
      </div>
    </div>
  );
}
