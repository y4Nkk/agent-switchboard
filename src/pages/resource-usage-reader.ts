import type { ResourceRead, ResourceReadRequest, ResourceSelection } from "../api/resource-usage.ts";
import type { createReadQueue } from "../hooks/use-read-queue.ts";

interface ReaderOptions {
  enqueue: ReturnType<typeof createReadQueue>;
  read: (request: ResourceReadRequest) => Promise<ResourceRead>;
  selection: () => ResourceSelection | null;
  onLoading: (loading: boolean) => void;
  onRead: (read: ResourceRead, selection: ResourceSelection | null) => void;
  onError: (error: unknown) => void;
}

/** A visible lifetime owns all entry, selection, manual and periodic reads. */
export function startResourceReading(options: ReaderOptions) {
  let active = true;
  let entering = true;
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let cancelPending: (() => void) | undefined;
  const schedule = (delay: number) => {
    if (active) timer = setTimeout(load, Math.max(100, Math.min(delay, 5_000)));
  };
  const load = () => {
    if (!active) return;
    clearTimeout(timer);
    cancelPending?.();
    const current = ++generation;
    const selection = options.selection();
    options.onLoading(true);
    options.onError(null);
    cancelPending = options.enqueue(async () => {
      if (!active || current !== generation) return;
      try {
        const read = await options.read({ resetInterval: entering, details: selection });
        if (!active || current !== generation) return;
        entering = false;
        options.onRead(read, selection);
        schedule(read.nextReadInMs);
      } catch (error) {
        if (!active || current !== generation) return;
        options.onError(error);
        schedule(5_000);
      } finally {
        if (active && current === generation) options.onLoading(false);
      }
    }, (error) => {
      if (!active || current !== generation) return;
      options.onError(error);
      options.onLoading(false);
      schedule(5_000);
    });
  };
  load();
  return {
    refresh: load,
    stop() {
      active = false;
      generation += 1;
      clearTimeout(timer);
      cancelPending?.();
    },
  };
}
