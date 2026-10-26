import { useCallback, useState } from "react";
import {
  commitCodexProfileSave, commitProfileSave, createCodexProfile, prepareCodexProfileSave, prepareProfileSave,
  type AppKind, type CodexProviderDraft, type CommandError, type ProviderDraft,
} from "../api/client";
import { useProfileSavePreparation } from "../components/useProfileSavePreparation";
import type { ProviderEditorSession, ProvidersDeps, SetEditorSession } from "./useProviders";

type Preparation = ReturnType<typeof useProfileSavePreparation>;
type Saved = (app: AppKind, id: string) => Promise<void>;

/** Claude and Codex official profiles share the same generic save boundary. */
function useGenericSave(deps: ProvidersDeps, session: ProviderEditorSession | null, pending: Preparation, saved: Saved) {
  const { busy, setBusy, clearError, onError, invalidateCandidates } = deps;
  return useCallback(async (draft: ProviderDraft) => {
    if (busy || !session || session.app !== draft.app) return;
    if (session.app === "codex" && session.source?.kind !== "official") return;
    const request = pending.checkpoint();
    const record = session.app === "claude" ? session.record
      : session.source?.kind === "official" ? session.source.record : null;
    setBusy(true);
    clearError();
    try {
      const prepared = await prepareProfileSave(record?.profile.id ?? null, draft, record?.fileHash ?? null, "save");
      if (prepared.kind === "saveAndApply") {
        if (!prepared.preview) {
          await pending.discard({ source: "profile", preparationId: prepared.preparationId });
          throw { code: "profile-preview-missing", message: "", messageKey: "operations.save.previewMissing" };
        }
        await pending.hold({ source: "profile", app: draft.app, preparationId: prepared.preparationId, preview: prepared.preview }, request);
        return;
      }
      const result = await commitProfileSave(prepared.preparationId, false);
      if (prepared.kind !== "noChange") invalidateCandidates();
      await saved(result.profile.app, result.profile.id);
    } catch (cause) { onError(cause as CommandError); }
    finally { setBusy(false); }
  }, [busy, session, setBusy, clearError, onError, invalidateCandidates, pending, saved]);
}

/** Codex third-party records keep their strict store and draft contract. */
function useCodexSave(deps: ProvidersDeps, session: ProviderEditorSession | null, pending: Preparation, saved: Saved) {
  const { busy, setBusy, clearError, onError, invalidateCandidates } = deps;
  return useCallback(async (draft: CodexProviderDraft) => {
    if (busy || session?.app !== "codex" || session.source?.kind === "official") return;
    const request = pending.checkpoint();
    setBusy(true);
    clearError();
    try {
      const record = session.source?.kind === "record" ? session.source.record : null;
      if (!record) {
        const created = await createCodexProfile(draft);
        invalidateCandidates();
        await saved("codex", created.profile.id);
        return;
      }
      const prepared = await prepareCodexProfileSave(record.profile.id, draft, record.fileHash, "save");
      if (prepared.kind === "saveAndApply") {
        if (!prepared.preview) {
          await pending.discard({ source: "codex", preparationId: prepared.preparationId });
          throw { code: "codex-profile-save-stale", message: "", messageKey: "operations.save.codexPreviewMissing" };
        }
        await pending.hold({ source: "codex", app: "codex", preparationId: prepared.preparationId, preview: prepared.preview }, request);
        return;
      }
      const result = await commitCodexProfileSave(prepared.preparationId, false);
      if (prepared.kind !== "noChange") invalidateCandidates();
      await saved("codex", result.profile.id);
    } catch (cause) { onError(cause as CommandError); }
    finally { setBusy(false); }
  }, [busy, session, setBusy, clearError, onError, invalidateCandidates, pending, saved]);
}

export function useProviderSaves(deps: ProvidersDeps, session: ProviderEditorSession | null, setSession: SetEditorSession) {
  const [pendingSaveError, setPendingSaveError] = useState<unknown>(null);
  const { busy, setBusy, clearError, invalidateCandidates, onError, setAppFilter, refresh, setTargetProfile } = deps;
  const report = useCallback((cause: unknown) => onError(cause as CommandError), [onError]);
  const preparation = useProfileSavePreparation(deps.active, report);
  const saved = useCallback(async (app: AppKind, id: string) => {
    setAppFilter(app);
    setSession(null);
    await refresh();
    await setTargetProfile(id);
  }, [setAppFilter, setSession, refresh, setTargetProfile]);
  const saveGenericProfile = useGenericSave(deps, session, preparation, saved);
  const saveCodexProfile = useCodexSave(deps, session, preparation, saved);
  const runPendingSave = useCallback(async () => {
    if (busy) return;
    const pendingSave = preparation.take();
    if (!pendingSave) return;
    setBusy(true);
    clearError();
    setPendingSaveError(null);
    try {
      const result = pendingSave.source === "codex" ? await commitCodexProfileSave(pendingSave.preparationId, true)
        : await commitProfileSave(pendingSave.preparationId, true);
      invalidateCandidates();
      await saved(pendingSave.app, result.profile.id);
    } catch (cause) { onError(cause as CommandError); }
    finally { preparation.finish(); setBusy(false); }
  }, [busy, preparation, setBusy, clearError, invalidateCandidates, onError, saved]);
  const cancelPendingSave = async () => {
    if (busy) return;
    setBusy(true);
    setPendingSaveError(null);
    try { await preparation.cancel(); }
    catch (cause) { setPendingSaveError(cause); report(cause); }
    finally { setBusy(false); }
  };
  return { pendingSave: preparation.pending, pendingSaveError, cancelPendingSave, saveGenericProfile, saveCodexProfile, runPendingSave };
}
