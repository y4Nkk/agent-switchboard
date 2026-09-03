import { useCallback, useEffect, useRef, useState } from "react";
import {
  getCommonSettingsEditor,
  previewCommonSettings,
  saveCommonSettings,
  type AppKind,
  type CommandError,
  type CommonSettingsEditor,
  type CommonSettingsPreview,
  type CommonValue,
} from "../api/client";

export type CommonSettingsPhase =
  | "idle"
  | "loading"
  | "loadError"
  | "clean"
  | "dirty"
  | "saving"
  | "saveError"
  | "savedPendingReapply";

export interface CommonSettingsEditorState {
  phase: CommonSettingsPhase;
  editor?: CommonSettingsEditor;
  draft?: Record<string, CommonValue>;
  error?: CommandError;
  preview?: CommonSettingsPreview;
  previewing?: boolean;
  previewError?: CommandError;
}

interface CommonSettingsDeps {
  busy: boolean;
  /** The page loads only when it becomes visible. */
  active: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  /** Saved values change every supplier projection candidate. */
  invalidateSwitchCandidates: () => void;
  refresh: () => Promise<void>;
  setBusy: (busy: boolean) => void;
}

function sameValues(
  left: Record<string, CommonValue>,
  right: Record<string, CommonValue>,
): boolean {
  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every((key) => {
      const before = left[key];
      const after = right[key];
      return (
        before.mode === after.mode &&
        (before.mode === "automatic" ||
          (after.mode === "explicit" && before.value === after.value))
      );
    })
  );
}

function phaseForDraft(
  editor: CommonSettingsEditor,
  draft: Record<string, CommonValue>,
  prior: CommonSettingsPhase,
): CommonSettingsPhase {
  if (!sameValues(draft, editor.settings.settings)) return "dirty";
  return prior === "savedPendingReapply" ? "savedPendingReapply" : "clean";
}

/**
 * General-settings state lives entirely in the application store. It owns
 * one independent automatic-or-explicit draft per client. Its preview is a
 * separately rendered common-settings fragment, never a client-file
 * candidate.
 */
export function useCommonSettings({
  busy,
  active,
  onError,
  clearError,
  invalidateSwitchCandidates,
  refresh,
  setBusy,
}: CommonSettingsDeps) {
  const [settingsApp, setSettingsApp] = useState<AppKind>("codex");
  const [states, setStates] = useState<Record<AppKind, CommonSettingsEditorState>>({
    codex: { phase: "idle" },
    claude: { phase: "idle" },
  });
  const loadRequests = useRef(new Map<AppKind, Promise<CommonSettingsEditor>>());

  const loadEditor = useCallback((app: AppKind, preserveDraft = false) => {
    const existing = loadRequests.current.get(app);
    if (existing) return existing;
    setStates((current) => ({
      ...current,
      [app]: { ...current[app], phase: "loading", error: undefined },
    }));
    const request = getCommonSettingsEditor(app);
    loadRequests.current.set(app, request);
    void request
      .then((editor) => {
        setStates((current) => {
          const prior = current[app];
          const draft =
            preserveDraft && prior.draft ? prior.draft : editor.settings.settings;
          return {
            ...current,
            [app]: {
              phase: phaseForDraft(editor, draft, prior.phase),
              editor,
              draft,
            },
          };
        });
      })
      .catch((caught) => {
        setStates((current) => ({
          ...current,
          [app]: { ...current[app], phase: "loadError", error: caught as CommandError },
        }));
      })
      .finally(() => {
        if (loadRequests.current.get(app) === request) loadRequests.current.delete(app);
      });
    return request;
  }, []);

  useEffect(() => {
    if (!active || states[settingsApp].phase !== "idle") return;
    void loadEditor(settingsApp);
  }, [active, loadEditor, settingsApp, states]);

  const changeValue = useCallback(
    (app: AppKind, key: string, value: CommonValue) => {
      if (busy) return;
      setStates((current) => {
        const state = current[app];
        if (!state.editor || !state.draft || state.phase === "saving") return current;
        const draft = { ...state.draft, [key]: value };
        return {
          ...current,
          [app]: {
            ...state,
            phase: phaseForDraft(state.editor, draft, state.phase),
            draft,
            error: undefined,
            preview: undefined,
            previewing: false,
            previewError: undefined,
          },
        };
      });
    },
    [busy],
  );

  /** Restores one group (or every group when group is null) to the client's
   * own automatic behavior. The draft keeps the edits unsaved. */
  const resetGroupToDefaults = useCallback(
    (app: AppKind, group: string | null) => {
      if (busy) return;
      setStates((current) => {
        const state = current[app];
        if (!state.editor || !state.draft || state.phase === "saving") return current;
        const draft = { ...state.draft };
        for (const spec of state.editor.specs) {
          if (group !== null && spec.group !== group) continue;
          draft[spec.key] = { mode: "automatic" };
        }
        return {
          ...current,
          [app]: {
            ...state,
            phase: phaseForDraft(state.editor, draft, state.phase),
            draft,
            error: undefined,
            preview: undefined,
            previewing: false,
            previewError: undefined,
          },
        };
      });
    },
    [busy],
  );

  const saveSettings = useCallback(
    async (app: AppKind) => {
      const current = states[app];
      if (
        busy ||
        !current.editor ||
        !current.draft ||
        (current.phase !== "dirty" && current.phase !== "saveError")
      ) {
        return;
      }
      setStates((all) => ({ ...all, [app]: { ...all[app], phase: "saving", error: undefined } }));
      setBusy(true);
      clearError();
      try {
        const saved = await saveCommonSettings(
          app,
          { settings: current.draft },
          current.editor.settingsHash,
        );
        setStates((all) => ({
          ...all,
          [app]: {
            phase: "savedPendingReapply",
            editor: {
              ...current.editor!,
              settings: saved.settings,
              settingsHash: saved.settingsHash,
            },
            draft: saved.settings.settings,
          },
        }));
        invalidateSwitchCandidates();
        void refresh().catch((caught) => onError(caught as CommandError));
      } catch (caught) {
        const error = caught as CommandError;
        setStates((all) => ({
          ...all,
          [app]: { ...all[app], phase: "saveError", error },
        }));
        onError(error);
      } finally {
        setBusy(false);
      }
    },
    [
      busy,
      clearError,
      invalidateSwitchCandidates,
      onError,
      refresh,
      setBusy,
      states,
    ],
  );

  const retryLoad = useCallback(
    (app: AppKind) => {
      if (busy) return;
      void loadEditor(app, states[app].draft !== undefined);
    },
    [busy, loadEditor, states],
  );

  /** Renders the current unsaved draft on demand. A response is ignored if
   * the draft changed while the read-only request was in flight. */
  const previewSettings = useCallback(
    (app: AppKind) => {
      const state = states[app];
      if (busy || state.previewing || !state.draft) return;
      const draft = state.draft;
      setStates((current) => ({
        ...current,
        [app]: { ...current[app], previewing: true, previewError: undefined },
      }));
      void previewCommonSettings(app, { settings: draft })
        .then((preview) => {
          setStates((current) => {
            const latest = current[app];
            if (!latest.draft || !sameValues(latest.draft, draft)) return current;
            return { ...current, [app]: { ...latest, preview, previewing: false } };
          });
        })
        .catch((caught) => {
          setStates((current) => {
            const latest = current[app];
            if (!latest.draft || !sameValues(latest.draft, draft)) return current;
            return {
              ...current,
              [app]: { ...latest, previewing: false, previewError: caught as CommandError },
            };
          });
        });
    },
    [busy, states],
  );

  return {
    settingsApp,
    setSettingsApp,
    editorState: states[settingsApp],
    changeValue,
    resetGroupToDefaults,
    saveSettings,
    retryLoad,
    previewSettings,
  };
}
