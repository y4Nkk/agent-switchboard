import { invoke } from "./client";
import type { AppKind } from "./shared";
import type { SessionIssue, SessionMeta } from "./sessions";

export type SessionRangeAction = "archive" | "unarchive" | "delete";
export interface SessionRangeRequest {
  app: AppKind | null; archived: boolean; activeAfter: string; activeBefore: string;
}
export interface SessionRangePreview {
  id: string; action: SessionRangeAction; request: SessionRangeRequest; matched: number;
  items: Array<{ session: SessionMeta; related: boolean }>; issues: SessionIssue[];
}
export interface SessionRangeProgress {
  id: string | null;
  running: boolean; completed: number; total: number; warning: string | null;
  outcomes: Array<{ app: AppKind; sessionId: string; completed: boolean; error: string | null }>;
}
export const previewSessionRange = (request: SessionRangeRequest, action: SessionRangeAction): Promise<SessionRangePreview> =>
  invoke("preview_session_range", { request, action });
export const discardSessionRange = (id: string): Promise<void> => invoke("discard_session_range", { id });
export const executeSessionRange = (id: string, confirmed: boolean): Promise<SessionRangeProgress> =>
  invoke("execute_session_range", { id, confirmed });
export const readSessionRangeProgress = (id: string): Promise<SessionRangeProgress | null> => invoke("read_session_range_progress", { id });
