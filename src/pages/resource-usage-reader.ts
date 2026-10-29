import type { ResourceRead, ResourceReadRequest, ResourceSelection } from "../api/resource-usage.ts";
import type { createReadQueue } from "../hooks/use-read-queue.ts";

export type ResourceReadActivity = "initial" | "manual" | "details" | null;
type ReadReason = Exclude<ResourceReadActivity, null> | "periodic";

interface ReaderOptions {
  enqueue: ReturnType<typeof createReadQueue>;
  read: (request: ResourceReadRequest) => Promise<ResourceRead>;
  selection: () => ResourceSelection | null;
  onActivity: (activity: ResourceReadActivity) => void;
  onRead: (read: ResourceRead, selection: ResourceSelection | null, memoryMaximum: number | null) => void;
  onError: (error: unknown) => void;
}

function memoryMaximum(read: ResourceRead, previous: number | null): number | null {
  const values = read.groups.flatMap((group) => group.residentBytes.value === null ? [] : [group.residentBytes.value]);
  if (!values.length) return previous;
  return Math.max(previous ?? 0, 2 ** Math.ceil(Math.log2(Math.max(1024 ** 2, ...values))));
}

/** A visible lifetime owns reads; only foreground intents publish activity. */
export function startResourceReading(options: ReaderOptions) {
  let active = true;
  let entering = true;
  let generation = 0;
  let maximum: number | null = null;
  let activity: ResourceReadActivity = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let cancelPending: (() => void) | undefined;
  const setActivity = (next: ResourceReadActivity) => {
    if (activity !== next) { activity = next; options.onActivity(next); }
  };
  const schedule = (delay: number) => {
    if (active) timer = setTimeout(() => load("periodic"), Math.max(100, Math.min(delay, 5_000)));
  };
  const failed = (current: number, error: unknown) => {
    if (!active || current !== generation) return;
    options.onError(error);
    setActivity(null);
    schedule(5_000);
  };
  const load = (reason: ReadReason) => {
    if (!active) return;
    clearTimeout(timer);
    cancelPending?.();
    const current = ++generation;
    const selection = options.selection();
    if (reason !== "periodic" && activity !== "manual") setActivity(reason);
    cancelPending = options.enqueue(async () => {
      if (!active || current !== generation) return;
      const resetInterval = entering;
      try {
        const read = await options.read({ resetInterval, details: selection });
        entering = false;
        if (!active || current !== generation) return;
        maximum = memoryMaximum(read, maximum);
        const knownMemory = read.groups.some((group) => group.residentBytes.value !== null);
        options.onRead(read, selection, knownMemory ? maximum : null);
        options.onError(null);
        schedule(read.nextReadInMs);
      } catch (error) { failed(current, error); }
      finally { if (active && current === generation) setActivity(null); }
    }, (error) => failed(current, error));
  };
  load("initial");
  return {
    refresh: () => load("manual"),
    select: () => load("details"),
    stop() {
      active = false;
      generation += 1;
      clearTimeout(timer);
      cancelPending?.();
      setActivity(null);
    },
  };
}
