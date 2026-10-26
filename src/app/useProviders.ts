import { useProviderSaves } from "./useProviderSaves";
import { useCallback, useState, type Dispatch, type SetStateAction } from "react";
import {
  cancelProfileSave,
  commitCodexProfileSave,
  commitProfileSave,
  deleteCodexProfile,
  deleteProfile,
  prepareCodexProfileSave,
  prepareProfileSave,
  reorderClaudeProfiles,
  type AppKind,
  type CommandError,
  type CodexProviderRecord,
  type ProviderDraft,
  type ProviderProfile,
  type ProviderRecord,
  type UsageQuery,
} from "../api/client";
import { codexProviderDraft } from "../lib/provider-drafts";

/** What a Codex editor session edits: a stored third-party record or the
 * client's official-login record. The variants never convert into each other;
 * only their shared editor shape is editable. */
export type CodexEditorSource =
  | { kind: "record"; record: CodexProviderRecord }
  | { kind: "official"; record: ProviderRecord | null };

/** One open editor. Claude owns the generic contract; Codex owns its strict
 * specialized contract — the two draft shapes never convert into each other. */
export type ProviderEditorSession =
  | { app: "claude"; record: ProviderRecord | null }
  | { app: "codex"; source: CodexEditorSource | null };

function codexSessionRecord(session: ProviderEditorSession | null): CodexProviderRecord | null {
  if (session?.app !== "codex") return null;
  return session.source?.kind === "record" ? session.source.record : null;
}

export type SetEditorSession = Dispatch<SetStateAction<ProviderEditorSession | null>>;

export interface ProvidersDeps {
  active: boolean;
  busy: boolean;
  appFilter: AppKind;
  setAppFilter: (app: AppKind) => void;
  records: ProviderRecord[];
  /** The Codex official-login record, stored in the same generic boundary. */
  codexOfficialRecords: ProviderRecord[];
  targetProfileId: string | null;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  invalidateCandidates: () => void;
  clearCandidates: () => void;
  refresh: () => Promise<void>;
  setTargetProfile: (profileId: string) => Promise<void> | void;
  setTargetProfileId: (id: string | null) => void;
}

function useProviderEditorSession(deps: ProvidersDeps) {
  const { appFilter, records, clearCandidates, setAppFilter, setTargetProfileId } = deps;
  const [editorSession, setEditorSession] = useState<ProviderEditorSession | null>(null);
  const openEditor = useCallback(
    (profile: ProviderProfile) => {
      const record = records.find((item) => item.profile.id === profile.id);
      if (!record) return;
      clearCandidates();
      setTargetProfileId(profile.id);
      setEditorSession({ app: "claude", record });
    },
    [records, clearCandidates, setTargetProfileId],
  );
  const openCodexEditor = useCallback(
    (record: CodexProviderRecord) => {
      clearCandidates();
      setTargetProfileId(record.profile.id);
      setEditorSession({ app: "codex", source: { kind: "record", record } });
    },
    [clearCandidates, setTargetProfileId],
  );
  /** Opens the Codex editor on the client's official-login record. A null
   * record creates it; the record itself is never a third-party draft. */
  const openCodexOfficialEditor = useCallback(
    (record: ProviderRecord | null) => {
      clearCandidates();
      setTargetProfileId(record?.profile.id ?? null);
      setAppFilter("codex");
      setEditorSession({ app: "codex", source: { kind: "official", record } });
    },
    [clearCandidates, setAppFilter, setTargetProfileId],
  );
  /** Switches an open Codex editor between its two access modes by replacing
   * the session; a third-party draft never mutates into an official record. */
  const switchCodexAccessMode = useCallback(
    (official: boolean, existing: ProviderRecord | null) => {
      clearCandidates();
      setTargetProfileId(existing?.profile.id ?? null);
      setEditorSession({
        app: "codex",
        source: official
          ? { kind: "official", record: existing }
          : null,
      });
    },
    [clearCandidates, setTargetProfileId],
  );
  const newEditor = useCallback(() => {
    clearCandidates();
    setEditorSession(
      appFilter === "codex"
        ? { app: "codex", source: null }
        : { app: "claude", record: null },
    );
  }, [appFilter, clearCandidates]);
  /** Cross-contract client switch: replace the session instead of mutating
   * one contract's draft into the other's. */
  const newEditorFor = useCallback((app: AppKind) => {
    clearCandidates();
    setAppFilter(app);
    setTargetProfileId(null);
    setEditorSession(
      app === "codex"
        ? { app: "codex", source: null }
        : { app: "claude", record: null },
    );
  }, [clearCandidates, setAppFilter, setTargetProfileId]);
  const closeEditor = useCallback(() => setEditorSession(null), []);
  /** Changing the list client clears the execution target and both preview states, not the editor session. */
  const selectApp = useCallback(
    (app: AppKind) => {
      clearCandidates();
      setAppFilter(app);
      setTargetProfileId(null);
    },
    [clearCandidates, setAppFilter, setTargetProfileId],
  );
  return { editorSession, setEditorSession, openEditor, openCodexEditor, openCodexOfficialEditor,
    switchCodexAccessMode, newEditor, newEditorFor, closeEditor, selectApp };
}
function useCodexMetadata(deps: ProvidersDeps) {
  const { busy, clearError, onError, refresh, setBusy } = deps;
  /** Saves one Codex third-party profile's usage query through the strict
   * Codex store. Usage metadata never re-applies the client route: a
   * save-and-apply classification means the base record drifted and is
   * rejected instead of silently applying. */
  const saveCodexProfileUsageQuery = useCallback(
    async (record: CodexProviderRecord, usageQuery: UsageQuery | null): Promise<boolean> => {
      if (busy) return false;
      setBusy(true);
      clearError();
      try {
        const draft = { ...codexProviderDraft(record), usageQuery };
        const prepared = await prepareCodexProfileSave(
          record.profile.id,
          draft,
          record.fileHash,
          "save",
        );
        if (prepared.kind === "saveAndApply") {
          await cancelProfileSave("codex", prepared.preparationId);
          throw {
            code: "profile-save-invalid",
            message: "", messageKey: "operations.save.patchNoApply",
          };
        }
        await commitCodexProfileSave(prepared.preparationId, false);
        await refresh();
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, refresh, setBusy],
  );

  return { saveCodexProfileUsageQuery };
}

function useProviderMetadata(deps: ProvidersDeps) {
  const { busy, clearError, onError, records, refresh, setBusy } = deps;
  const dragReorderClaudeProfiles = useCallback(async (orderedIds: string[]) => {
    if (busy) return;
    const expectedFileHashes = Object.fromEntries(records.map((record) => [record.profile.id, record.fileHash]));
    setBusy(true);
    clearError();
    try {
      await reorderClaudeProfiles(orderedIds, expectedFileHashes);
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, clearError, onError, records, refresh, setBusy]);
  /** Persists one metadata-only patch (usage query, quota interval) over a
   * stored profile. Only application-side fields may flow through here: a
   * patch must never re-apply the client configuration. */
  const saveProfilePatch = useCallback(
    async (profile: ProviderProfile, patch: Partial<ProviderDraft>): Promise<boolean> => {
      if (busy) return false;
      const record = records.find((candidate) => candidate.profile.id === profile.id);
      if (!record) return false;
      setBusy(true);
      clearError();
      try {
        const { id, ...draft } = profile;
        const prepared = await prepareProfileSave(id, { ...draft, ...patch }, record.fileHash, "save");
        if (prepared.kind === "saveAndApply") {
          await cancelProfileSave("profile", prepared.preparationId);
          throw {
            code: "profile-save-invalid",
            message: "", messageKey: "operations.save.patchNoApply",
          };
        }
        await commitProfileSave(prepared.preparationId, false);
        await refresh();
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, records, refresh, setBusy],
  );

  const saveProfileUsageQuery = useCallback(
    (profile: ProviderProfile, usageQuery: UsageQuery | null) =>
      saveProfilePatch(profile, { usageQuery }),
    [saveProfilePatch],
  );

  const { saveCodexProfileUsageQuery } = useCodexMetadata(deps);

  const saveOfficialQuotaInterval = useCallback(
    (profile: ProviderProfile, minutes: number) =>
      saveProfilePatch(profile, { officialQuotaRefreshIntervalMinutes: minutes > 0 ? minutes : null }),
    [saveProfilePatch],
  );
  return { dragReorderClaudeProfiles, saveProfileUsageQuery, saveCodexProfileUsageQuery, saveOfficialQuotaInterval };
}

/** One pending destructive provider removal. `generic` covers the Claude
 * providers and the Codex official-login record (both live in the generic
 * boundary); `codexThirdParty` covers the strict Codex store. Both clients
 * confirm through the same sheet; nothing deletes without it. */
export type PendingProviderRemoval =
  | { kind: "generic"; profile: ProviderProfile }
  | { kind: "codexThirdParty"; record: CodexProviderRecord };

function useProviderRemoval(deps: ProvidersDeps, setEditorSession: SetEditorSession) {
  const { busy, clearError, codexOfficialRecords, invalidateCandidates, onError, records, refresh,
    targetProfileId, setBusy, setTargetProfileId } = deps;
  const [deletePending, setDeletePending] = useState<PendingProviderRemoval | null>(null);
  const runDelete = useCallback(async () => {
    if (busy || !deletePending) return;
    const target = deletePending;
    if (target.kind === "codexThirdParty") {
      const id = target.record.profile.id;
      invalidateCandidates();
      setBusy(true);
      clearError();
      try {
        await deleteCodexProfile(id, target.record.fileHash);
        setEditorSession((current) =>
          codexSessionRecord(current)?.profile.id === id ? null : current);
        await refresh();
      } catch (caught) {
        onError(caught as CommandError);
      } finally {
        setDeletePending(null);
        setBusy(false);
      }
      return;
    }
    const id = target.profile.id;
    const record = [...records, ...codexOfficialRecords]
      .find((candidate) => candidate.profile.id === id);
    if (!record) {
      setDeletePending(null);
      onError({ code: "profile-not-found", message: "", messageKey: "operations.delete.missing" });
      return;
    }
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      await deleteProfile(id, record.fileHash);
      if (targetProfileId === id) {
        setTargetProfileId(null);
      }
      setEditorSession((current) =>
        current?.app === "codex" && current.source?.kind === "official"
          && current.source.record?.profile.id === id
          ? null
          : current?.app === "claude" && current.record?.profile.id === id
            ? null
            : current);
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setDeletePending(null);
      setBusy(false);
    }
  }, [busy, clearError, codexOfficialRecords, deletePending, invalidateCandidates, onError, records,
    refresh, targetProfileId, setBusy, setTargetProfileId, setEditorSession]);

  return { deletePending, setDeletePending, runDelete };
}

/** Editor identity remains independent of the current list selection and refreshed records. */
export function useProviders(deps: ProvidersDeps) {
  const { setEditorSession, ...editor } = useProviderEditorSession(deps);
  const saves = useProviderSaves(deps, editor.editorSession, setEditorSession);
  const metadata = useProviderMetadata(deps);
  const removal = useProviderRemoval(deps, setEditorSession);
  return { ...editor, ...saves, ...metadata, ...removal };
}
