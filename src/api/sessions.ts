import { invoke } from "./client";
import type { AppKind } from "./shared";
import type { LocalizedMessage } from "./switching";

export interface StorageReclaimOutcome { removed: number; warning: string | null }

/** Read-only local session metadata. The backend never exposes source paths. */
export interface SessionMeta {
  app: AppKind;
  sessionId: string;
  title: string;
  summary: string;
  projectDir: string | null;
  createdAt: string | null;
  lastActiveAt: string | null;
  resumeCommand: string | null;
  alias: string | null;
  pinned: boolean;
  tags: string[];
  parentSessionId: string | null;
  canDelete: boolean;
  preservation: { savedAt: string; sourceAvailable: boolean; needsAttention: boolean } | null;
}

export interface ClaudeSessionSource { path: string; available: boolean; removable: boolean; issue: string | null }

export const listClaudeSessionSources = (): Promise<ClaudeSessionSource[]> => invoke("claude_session_sources");
export const chooseClaudeSessionSource = (): Promise<ClaudeSessionSource[] | null> => invoke("pick_claude_session_source");
export const forgetClaudeSessionSource = (path: string): Promise<ClaudeSessionSource[]> => invoke("forget_claude_session_source", { path });

export interface SessionMessagePreview {
  id: string;
  role: string;
  content: string;
  contentLength: number;
  at: string | null;
  bookmarked: boolean | null;
  imageCount: number;
}

export interface SessionMessagePage {
  session: SessionMeta;
  items: SessionMessagePreview[];
  offset: number;
  total: number;
  pageSize: number;
  bookmarkIssue: LocalizedMessage | null;
  issues: string[];
}

export interface SessionContentPage {
  content: string;
  offset: number;
  total: number;
  pageSize: number;
  images: string[];
  issues: string[];
}

export type SessionCopySource = { kind: "session"; app: AppKind; sessionId: string; messageId: string }
  | { kind: "bookmark"; id: string };

export const getSessionCopyText = (source: SessionCopySource): Promise<string> => invoke("get_session_copy_text", { source });

export interface SessionIssue {
  app: AppKind;
  message: string;
}

export interface SessionSearchHit {
  session: SessionMeta;
  messageId: string | null;
  contentOffset: number;
  excerpt: string;
}

export interface SessionSearchPage {
  results: SessionSearchHit[];
  total: number;
  issues: SessionIssue[];
  projects: SessionProject[];
  tags: string[];
}

export interface SessionProject {
  app: AppKind;
  projectDir: string | null;
  /** Unique sessions in this project, scoped only by the selected client. */
  count: number;
}

export interface SessionSearchRequest {
  query: string;
  app: AppKind | null;
  offset: number;
  project: Pick<SessionProject, "app" | "projectDir"> | null;
  tag: string | null;
  pinnedOnly: boolean;
  /** Inclusive RFC3339 lower bound for lastActiveAt. */
  activeAfter: string | null;
  /** Exclusive RFC3339 upper bound for lastActiveAt. */
  activeBefore: string | null;
}

export type SessionOrganizationChange =
  | { kind: "pin"; pinned: boolean }
  | { kind: "alias"; alias: string | null }
  | { kind: "setTags" | "addTags" | "removeTags"; tags: string[] };

/** Result of starting a supported CLI's resume command in a new terminal. */
export interface SessionResume {
  command: string;
  usedProjectDir: boolean;
}

export function updateSessionOrganization(
  requests: SessionDeleteRequest[], change: SessionOrganizationChange,
): Promise<SessionMeta[]> {
  return invoke("update_session_organization", { requests, change });
}

export interface SessionBookmarkSummary {
  imageCount: number;
  id: string;
  app: AppKind;
  sessionId: string;
  sessionTitle: string;
  projectDir: string | null;
  messageId: string;
  role: string;
  excerpt: string;
  contentLength: number;
  contentOffset: number;
  at: string | null;
  savedAt: string;
}

export interface SessionBookmarkPage {
  items: SessionBookmarkSummary[];
  offset: number;
  total: number;
  pageSize: number;
}

export function exportSessionMarkdown(app: AppKind, sessionId: string): Promise<string | null> {
  return invoke("export_session_markdown", { app, sessionId });
}

export function searchSessionBookmarks(query: string, app: AppKind | null, offset: number): Promise<SessionBookmarkPage> {
  return invoke("search_session_bookmarks", { request: { query, app, offset } });
}

export function saveSessionBookmark(app: AppKind, sessionId: string, messageId: string): Promise<SessionBookmarkSummary> {
  return invoke("save_session_bookmark", { app, sessionId, messageId });
}

export function deleteSessionBookmark(id: string): Promise<StorageReclaimOutcome> {
  return invoke("delete_session_bookmark", { id });
}

export function getSessionMetadata(app: AppKind, sessionId: string): Promise<SessionMeta> {
  return invoke("get_session_metadata", { app, sessionId });
}

export function getSessionMessagePage(app: AppKind, sessionId: string, offset: number, anchorId: string | null): Promise<SessionMessagePage> {
  return invoke("get_session_message_page", { request: { app, sessionId, offset, anchorId } });
}

export function getSessionMessageContent(app: AppKind, sessionId: string, messageId: string, offset: number): Promise<SessionContentPage> {
  return invoke("get_session_message_content", { request: { app, sessionId, messageId, offset } });
}

export function getSessionBookmarkContent(id: string, offset: number): Promise<SessionContentPage> {
  return invoke("get_session_bookmark_content", { id, offset });
}

export function resumeSession(app: AppKind, sessionId: string): Promise<SessionResume> {
  return invoke<SessionResume>("resume_session", { app, sessionId });
}

/** Permanently removes the local session record the backend resolves itself. */
export function deleteSession(app: AppKind, sessionId: string): Promise<void> {
  return invoke<void>("delete_session", { app, sessionId });
}

export interface SessionDeleteRequest {
  app: AppKind;
  sessionId: string;
}

/** One line of a batch deletion: removed, or the reason the record stayed. */
export interface SessionDeleteOutcome {
  app: AppKind;
  sessionId: string;
  deleted: boolean;
  error: string | null;
}

/** Deletes several records against one backend scan; each item reports its own outcome. */
export function deleteSessions(requests: SessionDeleteRequest[]): Promise<SessionDeleteOutcome[]> {
  return invoke<SessionDeleteOutcome[]>("delete_sessions", { requests });
}

export interface SessionCleanupPlan { cutoff: string; sessions: number; bytes: number }
export interface SessionCleanupOutcome { removed: number; warnings: LocalizedMessage[] }

/** Previews how many preserved Claude sessions fall before the cutoff. */
export function previewSessionCleanup(cutoff: string): Promise<SessionCleanupPlan> {
  return invoke("preview_session_cleanup", { cutoff });
}

/** Purges preserved sessions older than the cutoff, including the raw client records. */
export function cleanupSessions(cutoff: string): Promise<SessionCleanupOutcome> {
  return invoke("cleanup_sessions", { cutoff });
}
