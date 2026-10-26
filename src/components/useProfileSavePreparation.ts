import { useCallback, useEffect, useRef, useState } from "react";
import { cancelProfileSave, type AppKind, type FilePreview, type ProfileSaveSource } from "../api/client";

export interface PendingProfileSave {
  source: ProfileSaveSource;
  app: AppKind;
  preparationId: string;
  preview: FilePreview;
}

/** Owns unconfirmed backend tokens; confirmed work is handed to the executor. */
export function useProfileSavePreparation(active: boolean, onError: (cause: unknown) => void) {
  const [pending, setPending] = useState<PendingProfileSave | null>(null);
  const held = useRef<PendingProfileSave | null>(null);
  const alive = useRef(false);
  const revision = useRef(0);
  const report = useRef(onError);
  useEffect(() => { report.current = onError; }, [onError]);
  const release = useCallback((value: Pick<PendingProfileSave, "source" | "preparationId">) =>
    cancelProfileSave(value.source, value.preparationId), []);
  const discard = useCallback((value: Pick<PendingProfileSave, "source" | "preparationId">) =>
    release(value).catch((cause) => report.current(cause)), [release]);
  useEffect(() => {
    alive.current = active;
    if (!active) setPending(null);
    return () => {
      alive.current = false;
      revision.current += 1;
      const value = held.current;
      held.current = null;
      if (value) void discard(value);
    };
  }, [active, discard]);
  const checkpoint = useCallback(() => revision.current, []);
  const isCurrent = useCallback((expected: number) => alive.current && revision.current === expected, []);
  const hold = useCallback(async (value: PendingProfileSave, expected: number) => {
    if (!isCurrent(expected) || held.current) {
      await discard(value);
      return;
    }
    held.current = value;
    setPending(value);
  }, [discard, isCurrent]);
  const take = useCallback(() => {
    const value = held.current;
    held.current = null;
    return value;
  }, []);
  const cancel = useCallback(async () => {
    const value = held.current;
    if (!value) return;
    await release(value);
    if (held.current === value) {
      held.current = null;
      setPending(null);
    }
  }, [release]);
  const finish = useCallback(() => setPending(null), []);
  return { pending, hold, take, cancel, discard, finish, checkpoint, isCurrent };
}
