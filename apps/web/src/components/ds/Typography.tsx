import { cn } from "@/lib/utils";

/**
 * Minimal replacement for `@nous-research/ui` Typography component.
 */
export function Typography({
  children,
  className,
  style,
  mondwest,
}: {
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
  mondwest?: boolean;
}) {
  return (
    <span
      className={cn(mondwest && "font-mondwest", className)}
      style={style}
    >
      {children}
    </span>
  );
}

/**
 * Minimal replacement for `@nous-research/ui` H2 component.
 */
export function H2({
  children,
  className,
  variant,
  mondwest: _mondwest,
  ...rest
}: {
  children: React.ReactNode;
  className?: string;
  variant?: "sm" | "md" | "lg";
  mondwest?: boolean;
} & Omit<React.HTMLAttributes<HTMLHeadingElement>, 'children'>) {
  const sizeClass =
    variant === "sm"
      ? "text-base"
      : variant === "lg"
        ? "text-2xl"
        : "text-xl";

  return (
    <h2 className={cn("font-bold tracking-wide", sizeClass, className)} {...rest}>
      {children}
    </h2>
  );
}
