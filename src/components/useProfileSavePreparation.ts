import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { cancelProfileSave, type AppKind, type FilePreview, type ProfileSaveSource } from "../api/client";
import type { ProviderSaveReview } from "../app/provider-save-review";

export interface ProfileSaveToken { source: ProfileSaveSource; app: AppKind; preparationId: string }
export interface PendingProfileSave extends ProfileSaveToken { preview: FilePreview; review: ProviderSaveReview; sessionRevision: string }
type Token = Pick<ProfileSaveToken, "source" | "preparationId">;
interface Release { token: Token; running?: Promise<void> }

/** Failed releases remain owned until a caller explicitly retries before preparing again. */
function useProfileSaveReleases(report: RefObject<(cause: unknown) => void>) {
  const queued = useRef(new Map<string, Release>());
  const [releaseError, setReleaseError] = useState<unknown>(null);
  const release = useCallback((value: Token) => cancelProfileSave(value.source, value.preparationId), []);
  const run = useCallback((entry: Release): Promise<void> => {
    if (entry.running) return entry.running;
    const key = `${entry.token.source}:${entry.token.preparationId}`;
    entry.running = release(entry.token).then(() => {
      if (queued.current.get(key) === entry) queued.current.delete(key);
      if (!queued.current.size) setReleaseError(null);
    }, (cause) => {
      entry.running = undefined;
      setReleaseError(cause);
      throw cause;
    });
    return entry.running;
  }, [release]);
  const discard = useCallback(async (token: Token) => {
    const key = `${token.source}:${token.preparationId}`;
    let entry = queued.current.get(key);
    if (!entry) { entry = { token }; queued.current.set(key, entry); }
    try { await run(entry); }
    catch (cause) { report.current(cause); }
  }, [run, report]);
  const retryReleases = useCallback(async () => {
    for (const entry of [...queued.current.values()]) await run(entry);
  }, [run]);
  return { release, discard, retryReleases, releaseError };
}

/** Owns unconfirmed backend tokens; confirmed work is handed to the executor. */
export function useProfileSavePreparation<T extends ProfileSaveToken = PendingProfileSave>(
  active: boolean, onError: (cause: unknown) => void, context = "",
) {
  const [pending, setPending] = useState<T | null>(null);
  const held = useRef<T | null>(null);
  const alive = useRef(false);
  const revision = useRef(0);
  const report = useRef(onError);
  const rendered = useRef({ active, context });
  const effectContext = useRef(context);
  rendered.current = { active, context };
  useEffect(() => { report.current = onError; }, [onError]);
  const releases = useProfileSaveReleases(report);
  const { release, discard } = releases;
  useEffect(() => {
    alive.current = active;
    effectContext.current = context;
    setPending(null);
    return () => {
      alive.current = false;
      revision.current += 1;
      const value = held.current;
      held.current = null;
      if (value) void discard(value);
    };
  }, [active, context, discard]);
  const checkpoint = useCallback(() => revision.current, []);
  const isCurrent = useCallback((expected: number) => alive.current && rendered.current.active
    && effectContext.current === rendered.current.context && revision.current === expected, []);
  const hold = useCallback(async (value: T, expected: number) => {
    if (!isCurrent(expected) || held.current) { await discard(value); return; }
    held.current = value;
    setPending(value);
  }, [discard, isCurrent]);
  const take = useCallback(() => {
    const value = held.current;
    held.current = null;
    if (!isCurrent(revision.current)) {
      if (value) void discard(value);
      setPending(null);
      return null;
    }
    return value;
  }, [discard, isCurrent]);
  const cancel = useCallback(async () => {
    const value = held.current;
    if (!value) return;
    await release(value);
    if (held.current === value) { held.current = null; setPending(null); }
  }, [release]);
  const finish = useCallback(() => setPending(null), []);
  const hasUnconfirmed = useCallback(() => held.current !== null, []);
  return { ...releases, pending, hold, take, cancel, finish, checkpoint, isCurrent, hasUnconfirmed };
}
