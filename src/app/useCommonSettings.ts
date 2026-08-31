import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyCommon,
  getCommon,
  getCommonChoices,
  getCommonToggles,
  previewCommon,
  setCommon,
  type AppKind,
  type CommandError,
  type CommonChoicesState,
  type CommonConfigPatch,
  type FilePreview,
  type ToggleState,
} from "../api/client";
import { notifyWarnings } from "./notifications";

interface CommonSettingsDeps {
  busy: boolean;
  /** Whether the settings page is the active page; previews only run then. */
  active: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  /** Writes from other domains invalidate the common candidate. */
  invalidateSwitchCandidates: () => void;
  refresh: () => Promise<void>;
  setBusy: (busy: boolean) => void;
}

/**
 * The general-configuration overlay domain: stored patch, live toggle and
 * choice state per client, and the settings-page candidate preview. Each
 * client owns its epoch so late responses never overwrite newer ones.
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
  const [commonPatches, setCommonPatches] = useState<Record<string, CommonConfigPatch>>({});
  const [toggles, setToggles] = useState<Record<string, ToggleState[]>>({});
  const [choices, setChoices] = useState<Record<string, CommonChoicesState>>({});
  const [commonPreview, setCommonPreview] = useState<Record<string, FilePreview | null>>({});
  const [settingsApp, setSettingsApp] = useState<AppKind>("codex");
  const commonEpoch = useRef<Record<AppKind, number>>({ codex: 0, claude: 0 });
  const commonPreviewRequests = useRef(new Map<AppKind, Promise<FilePreview>>());

  useEffect(() => {
    for (const app of ["codex", "claude"] as const) {
      void getCommon(app)
        .then((patch) => setCommonPatches((current) => ({ ...current, [app]: patch })))
        .catch((caught) => onError(caught as CommandError));
      void getCommonToggles(app)
        .then((list) => setToggles((current) => ({ ...current, [app]: list })))
        .catch((caught) => onError(caught as CommandError));
      void getCommonChoices(app)
        .then((state) => setChoices((current) => ({ ...current, [app]: state })))
        .catch((caught) => onError(caught as CommandError));
    }
  }, [onError]);

  const requestCommonPreview = useCallback((app: AppKind) => {
    const existing = commonPreviewRequests.current.get(app);
    if (existing) return existing;
    const next = previewCommon(app);
    commonPreviewRequests.current.set(app, next);
    void next
      .finally(() => {
        if (commonPreviewRequests.current.get(app) === next) commonPreviewRequests.current.delete(app);
      })
      .catch(() => {});
    return next;
  }, []);

  /** Writes invalidate every common candidate on both clients. */
  const invalidateCommonPreviews = useCallback(() => {
    commonEpoch.current.codex += 1;
    commonEpoch.current.claude += 1;
    commonPreviewRequests.current.clear();
    setCommonPreview({});
  }, []);

  /** The settings preview shows the candidate config file (pretty-printed)
   * plus its diff summary. It runs only for the active client after writes
   * settle, and each client owns its epoch. */
  useEffect(() => {
    if (!active || busy) return;
    const app = settingsApp;
    const version = ++commonEpoch.current[app];
    void requestCommonPreview(app)
      .then((preview) => {
        if (commonEpoch.current[app] === version) {
          setCommonPreview((current) => ({ ...current, [app]: preview }));
        }
      })
      .catch((caught) => {
        if (commonEpoch.current[app] === version) onError(caught as CommandError);
      });
  }, [busy, active, settingsApp, requestCommonPreview, onError]);

  /** One general-settings control = one config line: checkbox or slider.
   * Changes land through the executor's safe transaction immediately. */
  const applyCommonLine = useCallback(
    async (app: AppKind, key: string, value: boolean | string | null) => {
      if (busy) return;
      invalidateSwitchCandidates();
      invalidateCommonPreviews();
      setBusy(true);
      clearError();
      try {
        const base = commonPatches[app] ?? { app, entries: [] };
        const entries = base.entries.filter((entry) => entry.key !== key);
        entries.push({ key, value });
        const patch: CommonConfigPatch = { app, entries };
        await setCommon(app, patch);
        setCommonPatches((current) => ({ ...current, [app]: patch }));
        const outcome = await applyCommon(app, patch, true);
        if (outcome.warnings.length > 0) notifyWarnings("设置已写入", outcome.warnings);
        await refresh();
      } catch (caught) {
        onError(caught as CommandError);
      } finally {
        setBusy(false);
        const version = ++commonEpoch.current[app];
        // The controls must always reflect the real file, success or failure.
        void getCommonToggles(app)
          .then((list) => {
            if (commonEpoch.current[app] === version) {
              setToggles((current) => ({ ...current, [app]: list }));
            }
          })
          .catch(() => {});
        void getCommonChoices(app)
          .then((state) => {
            if (commonEpoch.current[app] === version) {
              setChoices((current) => ({ ...current, [app]: state }));
            }
          })
          .catch(() => {});
      }
    },
    [
      busy,
      clearError,
      commonPatches,
      invalidateCommonPreviews,
      invalidateSwitchCandidates,
      onError,
      refresh,
      setBusy,
    ],
  );

  return {
    settingsApp,
    setSettingsApp,
    toggles,
    choices,
    commonPreview,
    applyCommonLine,
    invalidateCommonPreviews,
  };
}
