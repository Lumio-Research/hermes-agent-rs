import { useState, useEffect } from "react";

/**
 * Minimal replacement for `@nous-research/ui/hooks/use-gpu-tier`.
 *
 * Returns 0 when the user prefers reduced motion or WebGL is unavailable,
 * otherwise returns 1. Used by Backdrop to gate the noise grain layer.
 */
export function useGpuTier(): number {
  const [tier, setTier] = useState(1);

  useEffect(() => {
    // Respect prefers-reduced-motion
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (mq.matches) {
      setTier(0);
      return;
    }

    // Quick WebGL probe
    try {
      const canvas = document.createElement("canvas");
      const gl =
        canvas.getContext("webgl2") || canvas.getContext("webgl");
      if (!gl) {
        setTier(0);
      }
    } catch {
      setTier(0);
    }
  }, []);

  return tier;
}
