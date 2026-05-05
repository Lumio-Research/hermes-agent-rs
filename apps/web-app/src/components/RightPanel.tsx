import { GitBranch, GitPullRequest, Globe, Terminal, FileCode2, ChevronDown } from "lucide-react";

interface RightPanelProps {
  /// Cloud agent / sandbox session id, used as the panel header.
  agentId?: string;
  /// Branch name shown in the "分支详情" section.
  branch?: string;
  /// Counts of code changes (additions/deletions) for a "更改" summary line.
  diffStats?: { additions: number; deletions: number };
  /// Click handlers for action rows. Omit to hide the row.
  onCreatePullRequest?: () => void;
  onOpenGitOperations?: () => void;
  onOpenComputerUse?: () => void;
  onOpenGithub?: () => void;
  onOpenWebSearch?: () => void;
}

/// Right-hand context panel for sandbox-backed sessions. Shows branch,
/// change summary, git operations, and a "Sources" section linking to other
/// tools the agent can pull context from. Pure layout — actions are wired
/// in by the parent via the optional handlers.
export function RightPanel({
  agentId,
  branch,
  diffStats,
  onCreatePullRequest,
  onOpenGitOperations,
  onOpenComputerUse,
  onOpenGithub,
  onOpenWebSearch,
}: RightPanelProps) {
  const hasDiff = !!diffStats && (diffStats.additions || diffStats.deletions);
  return (
    <aside className="hidden lg:flex w-[280px] shrink-0 flex-col border-l border-white/[0.05] bg-bg-secondary text-[13px] text-text-secondary">
      <div className="px-4 py-3 border-b border-white/[0.05]">
        <div className="text-[11px] uppercase tracking-wider text-text-muted">分支详情</div>
        {branch && (
          <div className="mt-1 flex items-center gap-1.5 text-text-primary">
            <GitBranch size={13} />
            <span className="font-mono text-[12px] truncate">{branch}</span>
          </div>
        )}
        {agentId && (
          <div className="mt-1 text-[11px] text-text-muted truncate" title={agentId}>
            agent: {agentId.slice(0, 24)}…
          </div>
        )}
      </div>

      {hasDiff && (
        <div className="px-4 py-3 border-b border-white/[0.05]">
          <div className="flex items-center justify-between">
            <span className="text-text-muted text-[12px]">更改</span>
            <span className="text-[12px] font-mono">
              <span className="text-emerald-300">+{diffStats!.additions}</span>
              <span className="text-text-muted"> / </span>
              <span className="text-rose-300">−{diffStats!.deletions}</span>
            </span>
          </div>
        </div>
      )}

      {(onOpenGitOperations || onCreatePullRequest) && (
        <div className="px-2 py-2 border-b border-white/[0.05]">
          {onOpenGitOperations && (
            <PanelRow icon={<FileCode2 size={13} />} label="Git 操作" onClick={onOpenGitOperations} />
          )}
          {onCreatePullRequest && (
            <PanelRow
              icon={<GitPullRequest size={13} />}
              label="Create pull request"
              onClick={onCreatePullRequest}
            />
          )}
        </div>
      )}

      {(onOpenComputerUse || onOpenGithub || onOpenWebSearch) && (
        <div className="px-4 pt-3 pb-1 text-[11px] uppercase tracking-wider text-text-muted">
          来源
        </div>
      )}
      <div className="px-2 pb-3">
        {onOpenComputerUse && (
          <PanelRow icon={<Terminal size={13} />} label="Computer Use" onClick={onOpenComputerUse} />
        )}
        {onOpenGithub && (
          <PanelRow icon={<GitBranch size={13} />} label="GitHub" onClick={onOpenGithub} />
        )}
        {onOpenWebSearch && (
          <PanelRow icon={<Globe size={13} />} label="网页搜索" onClick={onOpenWebSearch} />
        )}
      </div>

      <div className="mt-auto px-4 py-3 border-t border-white/[0.05] text-[11px] text-text-muted flex items-center justify-between">
        <span>面板</span>
        <ChevronDown size={11} />
      </div>
    </aside>
  );
}

function PanelRow({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick?: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-white/[0.05] hover:text-text-primary transition-colors text-left"
    >
      <span className="text-text-muted">{icon}</span>
      <span className="truncate">{label}</span>
    </button>
  );
}
