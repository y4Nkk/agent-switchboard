import { useEffect, useLayoutEffect, useRef, useState } from "react";

type ReturnFocus = () => HTMLElement | null;
type DialogState<T> = { value: T | null; returnFocus: ReturnFocus };
type RunPreview<T> = (action: () => Promise<T | undefined>) => Promise<T | undefined>;

export function useClientToolDialog<T>(active: boolean, busy: boolean) {
  const [state, setState] = useState<DialogState<T> | null>(null);
  const generation = useRef(0);

  useLayoutEffect(() => {
    if (!active) setState(null);
    return () => { generation.current += 1; };
  }, [active]);

  useEffect(() => {
    if (!active || busy || state?.value !== null) return;
    const target = state.returnFocus();
    setState(null);
    // Leave focus alone if the user moved it while the request was pending.
    if (target?.isConnected && target.ownerDocument.activeElement === target.ownerDocument.body) {
      target.focus();
    }
  }, [active, busy, state]);

  const open = (value: T, returnFocus: ReturnFocus) => {
    generation.current += 1;
    setState({ value, returnFocus });
  };
  const load = async (read: () => Promise<T>, run: RunPreview<T>, returnFocus: ReturnFocus) => {
    const request = ++generation.current;
    const value = await run(async () => {
      try {
        return await read();
      } catch (error) {
        if (generation.current === request) throw error;
      }
    });
    // The initial focus action has to be enabled on the dialog's first mount.
    if (generation.current === request) setState({ value: value ?? null, returnFocus });
  };
  const close = () => setState((current) => current && { ...current, value: null });
  return { dialog: active ? state?.value ?? null : null, open, load, close };
}
