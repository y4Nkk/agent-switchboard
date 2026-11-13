import { useCallback, useEffect, useRef, useState } from "react";
import {
  commitCodexProfileSave, commitProfileSave, createCodexProfile, prepareCodexProfileSave, prepareProfileSave,
  type AppKind, type CodexProviderDraft, type CommandError, type ProviderDraft,
} from "../api/client";
import { useProfileSavePreparation, type PendingProfileSave } from "../components/useProfileSavePreparation";
import type { ProviderEditorSession, ProvidersDeps, SetEditorSession } from "./useProviders";
import { routeDocument, type RouteInventory } from "../components/model-routing/model";
import { providerSaveReview } from "./provider-save-review";
import { restoreProviderEditorFocus } from "./provider-editor-focus";

type Preparation = ReturnType<typeof useProfileSavePreparation<PendingProfileSave>>;
type Saved = (app: AppKind, id: string, sessionRevision: string) => Promise<void>;

function editorIdentity(session: ProviderEditorSession | null): string {
  return session?.profileId ?? "closed";
}

function routeInventory(deps: ProvidersDeps): RouteInventory {
  return { codex: deps.codexRecords, profiles: [...deps.records, ...deps.codexOfficialRecords], activeIds: deps.activeIds };
}

/** Render-time identity also covers replacement with another blank editor. */
function useProviderSaveContext(deps: ProvidersDeps, session: ProviderEditorSession | null) {
  const inventory = JSON.stringify([deps.activeIds, [...deps.records, ...deps.codexOfficialRecords, ...deps.codexRecords].map((r) => [r.profile.id, r.fileHash])]);
  const current = useRef({ session, active: deps.active, app: deps.appFilter, inventory, revision: 0 });
  if (current.current.session !== session || current.current.active !== deps.active || current.current.app !== deps.appFilter || current.current.inventory !== inventory) {
    current.current = { session, active: deps.active, app: deps.appFilter, inventory, revision: current.current.revision + 1 };
  }
  const revision = current.current.revision;
  return {
    context: `${editorIdentity(session)}:${revision}`,
    isCurrent: () => current.current.session === session && current.current.active && current.current.app === deps.appFilter,
  };
}

/** Claude and Codex official profiles share the same generic save boundary. */
function useGenericSave(deps: ProvidersDeps, session: ProviderEditorSession | null, pending: Preparation, saved: Saved, sessionVersion: () => string) {
  const { busy, setBusy, clearError, onError, invalidateCandidates } = deps;
  const saving = useRef(false);
  return useCallback(async (draft: ProviderDraft) => {
    if (busy || saving.current || !session || session.app !== draft.app) return;
    if (session.app === "codex" && session.source?.kind !== "official") return;
    saving.current = true;
    const sessionRevision = sessionVersion();
    const request = pending.checkpoint();
    const record = session.app === "claude" ? session.record
      : session.source?.kind === "official" ? session.source.record : null;
    setBusy(true);
    clearError();
    try {
      await pending.retryReleases();
      if (!pending.isCurrent(request) || sessionRevision !== sessionVersion()) return;
      const target = record ? { kind: "update" as const, profileId: record.profile.id, expectedFileHash: record.fileHash }
        : { kind: "create" as const, profileId: session.profileId };
      const prepared = await prepareProfileSave(target, draft);
      if (!pending.isCurrent(request) || sessionRevision !== sessionVersion()) {
        await pending.discard({ source: "profile", preparationId: prepared.preparationId });
        return;
      }
      if (prepared.kind === "saveAndApply") {
        if (!prepared.preview || !record) {
          await pending.discard({ source: "profile", preparationId: prepared.preparationId });
          throw { code: "profile-preview-missing", message: "", messageKey: "operations.save.previewMissing" };
        }
        const before = routeDocument({ kind: "profile", record });
        const review = providerSaveReview(draft.app, before, { ...before, draft }, routeInventory(deps));
        await pending.hold({ source: "profile", app: draft.app, preparationId: prepared.preparationId, preview: prepared.preview, review, sessionRevision }, request);
        return;
      }
      const result = await commitProfileSave(prepared.preparationId, false);
      if (prepared.kind !== "noChange") invalidateCandidates();
      await saved(result.profile.app, result.profile.id, sessionRevision);
    } catch (cause) { onError(cause as CommandError); }
    finally { saving.current = false; setBusy(false); }
  }, [busy, session, setBusy, clearError, onError, invalidateCandidates, pending, saved, sessionVersion]);
}

/** Codex third-party records keep their strict store and draft contract. */
function useCodexSave(deps: ProvidersDeps, session: ProviderEditorSession | null, pending: Preparation, saved: Saved, sessionVersion: () => string) {
  const { busy, setBusy, clearError, onError, invalidateCandidates } = deps;
  const saving = useRef(false);
  return useCallback(async (draft: CodexProviderDraft) => {
    if (busy || saving.current || session?.app !== "codex" || session.source?.kind === "official") return;
    saving.current = true;
    const sessionRevision = sessionVersion();
    const request = pending.checkpoint();
    setBusy(true);
    clearError();
    try {
      await pending.retryReleases();
      if (!pending.isCurrent(request) || sessionRevision !== sessionVersion()) return;
      const record = session.source?.kind === "record" ? session.source.record : null;
      if (!record) {
        const created = await createCodexProfile(session.profileId, draft);
        invalidateCandidates();
        await saved("codex", created.profile.id, sessionRevision);
        return;
      }
      const prepared = await prepareCodexProfileSave(record.profile.id, draft, record.fileHash);
      if (!pending.isCurrent(request) || sessionRevision !== sessionVersion()) {
        await pending.discard({ source: "codex", preparationId: prepared.preparationId });
        return;
      }
      if (prepared.kind === "saveAndApply") {
        if (!prepared.preview) {
          await pending.discard({ source: "codex", preparationId: prepared.preparationId });
          throw { code: "codex-profile-save-stale", message: "", messageKey: "operations.save.codexPreviewMissing" };
        }
        const before = routeDocument({ kind: "codex", record });
        const review = providerSaveReview("codex", before, { ...before, draft }, routeInventory(deps));
        await pending.hold({ source: "codex", app: "codex", preparationId: prepared.preparationId, preview: prepared.preview, review, sessionRevision }, request);
        return;
      }
      const result = await commitCodexProfileSave(prepared.preparationId, false);
      if (prepared.kind !== "noChange") invalidateCandidates();
      await saved("codex", result.profile.id, sessionRevision);
    } catch (cause) { onError(cause as CommandError); }
    finally { saving.current = false; setBusy(false); }
  }, [busy, session, setBusy, clearError, onError, invalidateCandidates, pending, saved, sessionVersion]);
}

export function useProviderSaves(deps: ProvidersDeps, session: ProviderEditorSession | null, setSession: SetEditorSession, editorVersion: () => number) {
  const [pendingSaveError, setPendingSaveError] = useState<unknown>(null);
  const { busy, setBusy, clearError, invalidateCandidates, onError, setAppFilter, refresh, setTargetProfile } = deps;
  const report = useCallback((cause: unknown) => onError(cause as CommandError), [onError]);
  const saveContext = useProviderSaveContext(deps, session);
  const preparation = useProfileSavePreparation(deps.active, report, saveContext.context);
  const previousContext = useRef(saveContext.context);
  const unconfirmed = preparation.hasUnconfirmed();
  useEffect(() => {
    if (previousContext.current !== saveContext.context && deps.active && unconfirmed) {
      report({ code: "profile-save-stale", message: "", messageKey: "modelRoutes.contextChanged" });
    }
    previousContext.current = saveContext.context;
  }, [saveContext.context, deps.active, unconfirmed, report]);
  const sessionVersion = useCallback(() => `${editorVersion()}:${deps.workspaceVersion()}`, [editorVersion, deps.workspaceVersion]);
  const saved = useCallback(async (app: AppKind, id: string, sessionRevision: string) => {
    if (sessionRevision !== sessionVersion() || !saveContext.isCurrent()) { await refresh(); return; }
    setAppFilter(app);
    setSession(null);
    if (session?.returnTo === "gateway") deps.returnToGateway();
    const completedVersion = sessionVersion();
    // Select before awaiting the refresh, so a later response cannot select
    // over a replacement editor opened while the persisted list reloads.
    try { await setTargetProfile(id); }
    finally {
      await refresh();
      if (session?.returnTo === "providers" && completedVersion === sessionVersion()) restoreProviderEditorFocus(id, session.section);
    }
  }, [saveContext, session, sessionVersion, deps.returnToGateway, setAppFilter, setSession, refresh, setTargetProfile]);
  const saveGenericProfile = useGenericSave(deps, session, preparation, saved, sessionVersion);
  const saveCodexProfile = useCodexSave(deps, session, preparation, saved, sessionVersion);
  const runPendingSave = useCallback(async () => {
    if (busy) return;
    const pendingSave = preparation.take();
    if (!pendingSave) return;
    if (pendingSave.sessionRevision !== sessionVersion()) {
      await preparation.discard(pendingSave);
      preparation.finish();
      return;
    }
    setBusy(true);
    clearError();
    setPendingSaveError(null);
    try {
      const result = pendingSave.source === "codex" ? await commitCodexProfileSave(pendingSave.preparationId, true)
        : await commitProfileSave(pendingSave.preparationId, true);
      invalidateCandidates();
      await saved(pendingSave.app, result.profile.id, pendingSave.sessionRevision);
    } catch (cause) { onError(cause as CommandError); }
    finally { preparation.finish(); setBusy(false); }
  }, [busy, preparation, sessionVersion, setBusy, clearError, invalidateCandidates, onError, saved]);
  const cancelPendingSave = async () => {
    if (busy) return;
    setBusy(true);
    setPendingSaveError(null);
    try { await preparation.cancel(); }
    catch (cause) { setPendingSaveError(cause); report(cause); }
    finally { setBusy(false); }
  };
  return { pendingReleaseError: preparation.releaseError, pendingSave: preparation.pending, pendingSaveError, cancelPendingSave, saveGenericProfile, saveCodexProfile, runPendingSave };
}
