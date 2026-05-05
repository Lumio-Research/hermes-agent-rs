import { ZapIcon } from "lucide-react";

interface QuotaBannerProps {
  /// Usage percentage in [0, 100]. Banner only renders at >= 70.
  usagePct: number;
  /// Reset time hint (e.g. "重置于 6/12 04:26").
  resetHint?: string;
  /// Click handler for the upgrade CTA. If omitted, the button is hidden.
  onUpgrade?: () => void;
}

/// Inline quota banner shown at the top of an active chat once a tenant gets
/// close to / exceeds its monthly token budget. The component is layout-only
/// — copy is generic, no brand assets.
export function QuotaBanner({ usagePct, resetHint, onUpgrade }: QuotaBannerProps) {
  if (!Number.isFinite(usagePct) || usagePct < 70) return null;
  const exceeded = usagePct >= 100;
  const tone = exceeded
    ? "border-rose-500/40 bg-rose-500/10 text-rose-200"
    : "border-amber-500/40 bg-amber-500/10 text-amber-100";

  return (
    <div className={`mt-3 mx-auto max-w-4xl px-3 py-2 rounded-xl border ${tone} flex items-start gap-3 text-[12px]`}>
      <ZapIcon size={14} className="mt-0.5 shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="font-medium">
          {exceeded
            ? "本月用量已用尽"
            : `本月用量已使用 ${Math.round(usagePct)}%`}
        </div>
        {resetHint && (
          <div className="text-[11px] opacity-80 mt-0.5 truncate">{resetHint}</div>
        )}
      </div>
      {onUpgrade && (
        <button
          onClick={onUpgrade}
          className="shrink-0 px-3 py-1 rounded-md bg-white/10 hover:bg-white/20 text-[12px] font-medium transition-colors"
        >
          升级
        </button>
      )}
    </div>
  );
}
