import { useEffect, useRef } from "react";

interface RenderSanityState {
  windowStartedAt: number;
  count: number;
  warned: boolean;
}

export function useRenderSanityCheck(name: string, threshold = 60, windowMs = 1000): void {
  // Measure after commits to keep performance.now out of render.
  const stateRef = useRef<RenderSanityState | null>(null);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    if (stateRef.current === null) {
      stateRef.current = { windowStartedAt: performance.now(), count: 0, warned: false };
    }
    const state = stateRef.current;
    const now = performance.now();
    const elapsed = now - state.windowStartedAt;

    if (elapsed > windowMs) {
      state.windowStartedAt = now;
      state.count = 1;
      state.warned = false;
      return;
    }

    state.count += 1;
    if (!state.warned && state.count > threshold) {
      state.warned = true;
      console.error(
        `[render-sanity] ${name} rendered ${state.count} times in ${Math.round(elapsed)}ms. ` +
          `Likely cause: a useCallback/useMemo dep is getting a new reference every render ` +
          `(often an object/array state that's being reset unnecessarily, or a setState in an effect whose dep ` +
          `indirectly depends on that state). Check React DevTools' "Why did this render?" or add __track() diffs.`,
      );
    }
  });
}
