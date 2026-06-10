// Shared view-state hooks for the desktop shell.

import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Busy/error state around an async action: `run` sets busy, clears any
 * previous error, catches failures into `error`, and resets busy when done.
 */
export function useBusy() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  return { busy, error, run };
}

/** Flag that auto-resets after `ms` — e.g. "Copied" / "Saved" feedback. */
export function useFlash(ms = 1500): [boolean, () => void] {
  const [on, setOn] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const flash = useCallback(() => {
    setOn(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setOn(false), ms);
  }, [ms]);

  useEffect(() => () => clearTimeout(timer.current), []);

  return [on, flash];
}
