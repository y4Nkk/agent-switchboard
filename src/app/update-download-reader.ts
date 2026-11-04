import type { UpdateDownloadState } from "../api/updates.ts";
import { updateDownloadActive } from "./update-download-state.ts";

interface Options {
  read: (id: string) => Promise<UpdateDownloadState>;
  onRead: (state: UpdateDownloadState) => void;
  onError: (error: unknown) => void;
}

/** An application task keeps its reader across page navigation. */
export function observeUpdateDownload(id: string, options: Options) {
  let active = true;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = async () => {
    try {
      const next = await options.read(id);
      if (!active) return;
      options.onRead(next);
      options.onError(null);
      if (updateDownloadActive(next.phase)) timer = setTimeout(() => void tick(), 200);
    } catch (error) {
      if (!active) return;
      options.onError(error);
      timer = setTimeout(() => void tick(), 1_000);
    }
  };
  void tick();
  return () => { active = false; clearTimeout(timer); };
}
