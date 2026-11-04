import type { CodexProbeBatch } from "../api/codex-probe.ts";
import type { createReadQueue } from "../hooks/use-read-queue.ts";

interface Options {
  batchId: string;
  read: (id: string) => Promise<CodexProbeBatch | null>;
  enqueue: ReturnType<typeof createReadQueue>;
  onRead: (batch: CodexProbeBatch | null) => void;
  onError: (error: unknown) => void;
  onLoading: (loading: boolean) => void;
}

/** Only a visible detail owns answer payloads; old native reads may finish. */
export function startProbeDetailReading(options: Options) {
  let active = true;
  let generation = 0;
  let cancelPending: (() => void) | undefined;
  const fail = (version: number, error: unknown) => {
    if (!active || version !== generation) return;
    options.onError(error);
    options.onLoading(false);
  };
  const refresh = () => {
    if (!active) return;
    const version = ++generation;
    cancelPending?.();
    options.onLoading(true);
    cancelPending = options.enqueue(async () => {
      if (!active || version !== generation) return;
      try {
        const batch = await options.read(options.batchId);
        if (!active || version !== generation) return;
        options.onRead(batch);
      } catch (error) { fail(version, error); }
      finally { if (active && version === generation) options.onLoading(false); }
    }, (error) => fail(version, error));
  };
  refresh();
  return { refresh, stop() {
    active = false;
    generation += 1;
    cancelPending?.();
    options.onLoading(false);
  } };
}
