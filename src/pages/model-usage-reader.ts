import type { ModelUsageRead, ModelUsageRefresh } from "../api/model-usage.ts";

export type ModelUsageActivity = "snapshot" | "refresh" | null;
export type ModelUsageRunning = Extract<ModelUsageRefresh, { status: "running" }>;

interface ReaderOptions {
  readSnapshot: () => Promise<ModelUsageRead | null>;
  refresh: () => Promise<ModelUsageRefresh>;
  readRefresh: (id: string) => Promise<ModelUsageRefresh>;
  onRead: (read: ModelUsageRead) => void;
  onProgress: (task: ModelUsageRunning | null) => void;
  onActivity: (activity: ModelUsageActivity) => void;
  onError: (error: unknown) => void;
}

function refreshDelay(refreshAfter: string): number {
  const target = new Date(refreshAfter).getTime();
  return Number.isNaN(target) ? 30_000 : Math.max(1_000, target - Date.now());
}

/** The visible reader observes one backend-owned task; hiding never cancels its commit. */
export function startModelUsageReading(options: ReaderOptions) {
  let active = true;
  let entering = true;
  let refreshing = false;
  let observationFailures = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const schedule = (run: () => void, delay: number) => {
    clearTimeout(timer);
    if (active) timer = setTimeout(run, delay);
  };
  const finish = () => {
    refreshing = false;
    options.onProgress(null);
    options.onActivity(null);
  };
  const accept = (task: ModelUsageRefresh) => {
    if (!active) return;
    observationFailures = 0;
    if (task.status === "running") {
      options.onProgress(task);
      options.onError(null);
      schedule(() => void observe(task.id), 250);
    } else {
      finish();
      if (task.status === "completed") {
        options.onRead(task.read);
        options.onError(null);
        schedule(() => void refresh(), refreshDelay(task.read.refreshAfter));
      } else {
        options.onError(new Error(task.message));
        schedule(() => void refresh(), 30_000);
      }
    }
  };
  const observe = async (id: string) => {
    try { accept(await options.readRefresh(id)); }
    catch (error) {
      if (!active) return;
      options.onError(error);
      if (++observationFailures < 3) schedule(() => void observe(id), 1_000);
      else { finish(); schedule(() => void refresh(), 30_000); }
    }
  };
  const refresh = async () => {
    if (!active || entering || refreshing) return;
    clearTimeout(timer);
    refreshing = true;
    options.onProgress(null);
    options.onActivity("refresh");
    try { accept(await options.refresh()); }
    catch (error) {
      if (!active) return;
      finish();
      options.onError(error);
      schedule(() => void refresh(), 30_000);
    }
  };
  const enter = async () => {
    options.onActivity("snapshot");
    options.onError(null);
    try {
      const saved = await options.readSnapshot();
      if (active && saved) options.onRead(saved);
    } catch (error) { if (active) options.onError(error); }
    entering = false;
    if (active) await refresh();
  };
  void enter();
  return { refresh, stop() {
    active = false;
    clearTimeout(timer);
    options.onProgress(null);
    options.onActivity(null);
  } };
}
