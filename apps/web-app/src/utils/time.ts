/// Format an ISO timestamp as "刚刚 / N 分钟前 / N 小时前 / N 天前 / 日期".
export function formatRelativeTime(iso: string | undefined | null, now: Date = new Date()): string {
  if (!iso) return "";
  const ts = new Date(iso);
  if (Number.isNaN(ts.getTime())) return "";
  const diffMs = now.getTime() - ts.getTime();
  const diffSec = Math.max(0, Math.round(diffMs / 1000));
  if (diffSec < 30) return "刚刚";
  if (diffSec < 60) return `${diffSec} 秒前`;
  const diffMin = Math.round(diffSec / 60);
  if (diffMin < 60) return `${diffMin} 分钟前`;
  const diffHr = Math.round(diffMin / 60);
  if (diffHr < 24) return `${diffHr} 小时前`;
  const diffDay = Math.round(diffHr / 24);
  if (diffDay < 7) return `${diffDay} 天前`;
  return ts.toLocaleDateString();
}

/// Format a duration in milliseconds as "Xms / X.Ys / Xm Ys".
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(sec < 10 ? 1 : 0)}s`;
  const m = Math.floor(sec / 60);
  const s = Math.round(sec - m * 60);
  return `${m}m ${s}s`;
}
