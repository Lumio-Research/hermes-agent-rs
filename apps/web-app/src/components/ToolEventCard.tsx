import { useMemo } from "react";
import { Terminal, ChevronRight, Globe, Wrench, AlertCircle } from "lucide-react";
import { formatDuration } from "../utils/time";
import type { ExecutionTimelineEvent } from "../types";

interface ToolEventCardProps {
  events: ExecutionTimelineEvent[];
}

interface ToolGroup {
  key: string;
  tool: string;
  kind: "tool" | "status";
  status: "running" | "done" | "error";
  events: ExecutionTimelineEvent[];
  startedAt?: string;
  endedAt?: string;
}

function groupEvents(events: ExecutionTimelineEvent[]): ToolGroup[] {
  const groups: ToolGroup[] = [];
  const active = new Map<string, number>();
  for (const ev of events) {
    if (ev.type === "status") {
      groups.push({
        key: `status-${groups.length}`,
        tool: "status",
        kind: "status",
        status: "done",
        events: [ev],
        startedAt: ev.created_at,
        endedAt: ev.created_at,
      });
      continue;
    }
    const tool = ev.tool?.trim() || "tool";
    let idx = active.get(tool);
    const fresh = ev.type === "tool_start" || idx === undefined;
    if (fresh) {
      groups.push({
        key: `${tool}-${groups.length}`,
        tool,
        kind: "tool",
        status: ev.type === "tool_complete" ? "done" : "running",
        events: [],
        startedAt: ev.created_at,
      });
      idx = groups.length - 1;
      active.set(tool, idx);
    }
    const safeIdx = idx ?? groups.length - 1;
    const grp = groups[safeIdx];
    grp.events.push(ev);
    if (ev.type === "tool_complete") {
      grp.status = "done";
      grp.endedAt = ev.created_at;
      active.delete(tool);
    }
  }
  return groups;
}

function durationOf(group: ToolGroup): string {
  if (!group.startedAt || !group.endedAt) return "";
  const t0 = new Date(group.startedAt).getTime();
  const t1 = new Date(group.endedAt).getTime();
  if (Number.isNaN(t0) || Number.isNaN(t1)) return "";
  return formatDuration(Math.max(0, t1 - t0));
}

function iconFor(tool: string) {
  const t = tool.toLowerCase();
  if (t.includes("terminal") || t.includes("shell") || t.includes("exec")) return <Terminal size={12} />;
  if (t.includes("browser") || t.includes("web")) return <Globe size={12} />;
  return <Wrench size={12} />;
}

function statusPill(status: "running" | "done" | "error") {
  const cls =
    status === "done"
      ? "border-emerald-400/40 bg-emerald-400/10 text-emerald-200"
      : status === "error"
        ? "border-rose-400/40 bg-rose-400/10 text-rose-200"
        : "border-amber-400/40 bg-amber-400/10 text-amber-100 animate-pulse";
  const label = status === "done" ? "done" : status === "error" ? "error" : "running";
  return (
    <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[10px] ${cls}`}>
      {label}
    </span>
  );
}

/// Card-style tool execution timeline. One card per tool invocation, with a
/// summary header (icon · tool name · status pill · duration) and a
/// collapsible body containing the captured stdout/arguments.
export function ToolEventCard({ events }: ToolEventCardProps) {
  const groups = useMemo(() => groupEvents(events), [events]);
  if (!groups.length) return null;

  return (
    <div className="mt-2 space-y-1.5">
      {groups.map((group) => {
        if (group.kind === "status") {
          const ev = group.events[0];
          return (
            <div
              key={group.key}
              className="flex items-center gap-2 text-[11px] text-text-muted px-3 py-1.5 rounded-md bg-white/[0.02] border border-white/[0.04]"
            >
              <AlertCircle size={11} className="text-blue-300" />
              <span>{ev.content || "状态更新"}</span>
            </div>
          );
        }
        const dur = durationOf(group);
        return (
          <details
            key={group.key}
            className="rounded-lg border border-white/[0.06] bg-black/20 open:bg-black/30 transition-colors"
          >
            <summary className="cursor-pointer list-none flex items-center gap-2 px-3 py-2 text-[12px] text-text-secondary">
              <ChevronRight
                size={11}
                className="shrink-0 transition-transform [details[open]>&]:rotate-90"
              />
              {iconFor(group.tool)}
              <span className="font-mono text-text-primary truncate">{group.tool}</span>
              {statusPill(group.status)}
              {dur && <span className="text-text-muted text-[11px]">{dur}</span>}
              <span className="ml-auto text-text-muted text-[11px]">
                {group.events.length} event{group.events.length === 1 ? "" : "s"}
              </span>
            </summary>
            <div className="px-3 pb-2 space-y-1.5">
              {group.events.map((ev, idx) => (
                <div key={`${group.key}-${idx}`} className="text-[12px] text-text-secondary">
                  <div className="text-[10px] text-text-muted">
                    {ev.type}
                    {ev.chunk_index && ev.chunk_total
                      ? ` · ${ev.chunk_index}/${ev.chunk_total}`
                      : ""}
                  </div>
                  {ev.arguments && (
                    <pre className="mt-1 whitespace-pre-wrap break-all rounded bg-black/40 px-2 py-1 text-[11px] text-text-muted">
                      {ev.arguments}
                    </pre>
                  )}
                  {ev.content && (
                    <pre className="mt-1 max-h-56 overflow-auto whitespace-pre-wrap break-all rounded bg-black/40 px-2 py-1 text-[11px] text-text-secondary">
                      {ev.content}
                    </pre>
                  )}
                </div>
              ))}
            </div>
          </details>
        );
      })}
    </div>
  );
}
