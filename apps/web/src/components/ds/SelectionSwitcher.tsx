/**
 * Minimal replacement for `@nous-research/ui` SelectionSwitcher.
 *
 * The original overrides the browser's default text selection color
 * with a custom highlight. We replicate the effect with a simple
 * CSS injection — no runtime logic needed.
 */
export function SelectionSwitcher() {
  return (
    <style>{`
      ::selection {
        background: color-mix(in srgb, var(--midground-base, #e5e5e5) 30%, transparent);
        color: inherit;
      }
    `}</style>
  );
}
