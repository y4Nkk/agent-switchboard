import { invoke, type CommandError } from "./client";
import type { SessionSearchPage, SessionSearchRequest } from "./sessions";
import type { AppKind } from "./shared";

export interface SessionSearchProgress {
  stage: "discovering" | "indexing" | "searching";
  completed: number;
  total: number | null;
  messagesCompleted: number;
  messagesTotal: number | null;
}

export interface TranscriptMatch {
  messageId: string;
  contentRevision: string;
  contentOffset: number;
  start: number;
  end: number;
}

interface TranscriptSearchRequest {
  app: AppKind;
  sessionId: string;
  query: string;
  offset: number;
  revision: string | null;
}

export interface TranscriptSearchPage {
  hits: TranscriptMatch[];
  total: number;
  offset: number;
  pageSize: number;
  revision: string;
  issues: string[];
}

type SearchResult =
  | { scope: "library"; value: SessionSearchPage }
  | { scope: "transcript"; value: TranscriptSearchPage };

type SearchTaskState =
  | { state: "running"; progress: SessionSearchProgress }
  | { state: "completed"; result: SearchResult }
  | { state: "failed"; message: string }
  | { state: "expired" };

type SearchReadRequest = { scope: "library"; id: string; page: number } | { scope: "transcript"; id: string };
type ReportProgress = (progress: SessionSearchProgress) => void;

function pause(signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const abort = () => { window.clearTimeout(timer); reject(signal.reason); };
    const timer = window.setTimeout(() => { signal.removeEventListener("abort", abort); resolve(); }, 150);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) { signal.removeEventListener("abort", abort); abort(); }
  });
}

function searchFailed(message: string): CommandError {
  return { code: "session-search-failed", message,
    messageKey: "errors.sessions.searchFailed", params: { detail: message } };
}

function cancelSearch(id: string) {
  void invoke("cancel_session_search", { id }).catch((error) => console.warn("Session search cancellation failed", error));
}

async function readSearch(request: SearchReadRequest, signal: AbortSignal, onProgress: ReportProgress) {
  for (;;) {
    signal.throwIfAborted();
    const status = await invoke<SearchTaskState>("read_session_search", { request });
    signal.throwIfAborted();
    if (status.state === "expired") return null;
    if (status.state === "failed") throw searchFailed(status.message);
    if (status.state === "completed") {
      if (status.result.scope !== request.scope) throw new Error("Session search result scope mismatch");
      return status.result;
    }
    onProgress(status.progress);
    await pause(signal);
  }
}

/** The workspace owns one search across page changes. Closing it releases the backend snapshot. */
export function createSessionSearch(request: SessionSearchRequest) {
  let task: Promise<string> | undefined;
  const release = (owned: Promise<string>) => {
    task = undefined;
    void owned.then(cancelSearch, () => undefined);
  };
  return {
    async page(page: number, signal: AbortSignal, onProgress: ReportProgress): Promise<SessionSearchPage> {
      for (let attempt = 0; attempt < 2; attempt += 1) {
        signal.throwIfAborted();
        const owned = task ??= invoke<string>("start_session_search", { request: { scope: "library", value: request } });
        try {
          const id = await owned;
          const result = await readSearch({ scope: "library", id, page }, signal, onProgress);
          if (result?.scope === "library") return result.value;
          if (task === owned) task = undefined;
        } catch (error) {
          if (!signal.aborted && task === owned) release(owned);
          throw error;
        }
      }
      throw searchFailed("会话搜索已结束或停止，请重新搜索");
    },
    close() { if (task) release(task); },
  };
}

export async function searchTranscript(request: TranscriptSearchRequest, signal: AbortSignal,
  onProgress: ReportProgress): Promise<TranscriptSearchPage> {
  signal.throwIfAborted();
  const id = await invoke<string>("start_session_search", { request: { scope: "transcript", value: request } });
  let needsCancel = true;
  const cancel = () => { if (needsCancel) { needsCancel = false; cancelSearch(id); } };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    const result = await readSearch({ scope: "transcript", id }, signal, onProgress);
    needsCancel = false;
    if (result?.scope !== "transcript") throw searchFailed("会话搜索已结束或停止，请重新搜索");
    return result.value;
  } finally {
    signal.removeEventListener("abort", cancel);
    cancel();
  }
}
