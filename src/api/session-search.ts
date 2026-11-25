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
  | { state: "failed"; message: string };

function pause(signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const abort = () => { window.clearTimeout(timer); reject(signal.reason); };
    const timer = window.setTimeout(() => { signal.removeEventListener("abort", abort); resolve(); }, 150);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) { signal.removeEventListener("abort", abort); abort(); }
  });
}

/** Both native and browser development consume the same backend task lifecycle. */
async function runSearch(scope: "library", request: SessionSearchRequest, signal: AbortSignal,
  onProgress: (progress: SessionSearchProgress) => void): Promise<SessionSearchPage>;
async function runSearch(scope: "transcript", request: TranscriptSearchRequest, signal: AbortSignal,
  onProgress: (progress: SessionSearchProgress) => void): Promise<TranscriptSearchPage>;
async function runSearch(scope: SearchResult["scope"], request: SessionSearchRequest | TranscriptSearchRequest,
  signal: AbortSignal, onProgress: (progress: SessionSearchProgress) => void) {
  signal.throwIfAborted();
  const id = await invoke<string>("start_session_search", { request: { scope, value: request } });
  let consumed = false;
  let cancellationSent = false;
  const cancel = () => {
    if (cancellationSent) return;
    cancellationSent = true;
    void invoke("cancel_session_search", { id }).catch((error) => console.warn("Session search cancellation failed", error));
  };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    for (;;) {
      signal.throwIfAborted();
      const status = await invoke<SearchTaskState>("read_session_search", { id });
      signal.throwIfAborted();
      if (status.state === "completed") {
        consumed = true;
        if (status.result.scope !== scope) throw new Error("Session search result scope mismatch");
        return status.result.value;
      }
      if (status.state === "failed") {
        consumed = true;
        throw { code: "session-search-failed", message: status.message,
          messageKey: "errors.sessions.searchFailed", params: { detail: status.message } } satisfies CommandError;
      }
      onProgress(status.progress);
      await pause(signal);
    }
  } finally {
    signal.removeEventListener("abort", cancel);
    // Aborting before start returned still owns the newly allocated task.
    if (!consumed) cancel();
  }
}

export function searchSessions(request: SessionSearchRequest, signal: AbortSignal,
  onProgress: (progress: SessionSearchProgress) => void) {
  return runSearch("library", request, signal, onProgress);
}

export function searchTranscript(request: TranscriptSearchRequest, signal: AbortSignal,
  onProgress: (progress: SessionSearchProgress) => void) {
  return runSearch("transcript", request, signal, onProgress);
}
