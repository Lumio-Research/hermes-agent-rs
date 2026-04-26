import { cn } from "@/lib/utils";

/**
 * Minimal replacement for `@nous-research/ui` Grid component.
 * A CSS grid container with a thin border between cells.
 */
export function Grid({
  children,
  className,
  style,
}: {
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}) {
  return (
    <div
      className={cn(
        "grid grid-cols-1 border border-current/10",
        className,
      )}
      style={style}
    >
      {children}
    </div>
  );
}

/**
 * Minimal replacement for `@nous-research/ui` Cell component.
 * A grid cell with padding and a left border separator.
 */
export function Cell({
  children,
  className,
  style,
}: {
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}) {
  return (
    <div
      className={cn(
        "p-4 border-l border-current/10 first:border-l-0",
        className,
      )}
      style={style}
    >
      {children}
    </div>
  );
}
