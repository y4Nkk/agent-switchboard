import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  getAppSettings, onDesktopSettingsError, restartApplication, setAppSettings,
  type AppSettings, type AppSettingsSnapshot, type CommandError,
} from "../api/client";
import { applyAppAppearance } from "../lib/app-appearance";
import { applyLanguagePreference } from "../i18n/current";
import { isBrowserDevelopment } from "../lib/runtime";

interface AppSettingsDeps {
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
}

function useLoadedAppSettings(onError: AppSettingsDeps["onError"], revision: number) {
  const [appSettings, setAppSettingsState] = useState<AppSettings | null>(null);
  const [loadError, setLoadError] = useMessageState();
  const [desktopError, setDesktopError] = useMessageState();
  const [reload, setReload] = useState(0);
  const acceptSnapshot = useCallback((snapshot: AppSettingsSnapshot) => {
    applyLanguagePreference(snapshot.settings.language);
    setAppSettingsState(snapshot.settings);
    setDesktopError(snapshot.desktopError);
    setLoadError(null);
    if (snapshot.desktopError) onError(snapshot.desktopError);
  }, [onError]);
  useEffect(() => {
    let disposed = false;
    void getAppSettings().then((snapshot) => {
      if (!disposed) acceptSnapshot(snapshot);
    }).catch((caught: CommandError) => {
      if (!disposed) { onError(caught); setLoadError(caught); }
    });
    return () => { disposed = true; };
  }, [onError, reload, revision, acceptSnapshot]);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void onDesktopSettingsError((error) => {
      setDesktopError(error);
      onError(error);
    }).then((unlisten) => {
      if (disposed) unlisten(); else stop = unlisten;
    }).catch((error: unknown) => {
      if (!disposed) {
        setDesktopError(error);
        onError({ code: "desktop-settings-listen-failed", message: String(error) });
      }
    });
    return () => { disposed = true; stop?.(); };
  }, [onError]);
  useEffect(() => {
    applyAppAppearance(appSettings);
    // Mirror native main-window zoom in the development browser only.
    if (isBrowserDevelopment) document.documentElement.style.zoom = String((appSettings?.interfaceScale ?? 100) / 100);
  }, [appSettings]);
  const retryLoad = useCallback(() => setReload((count) => count + 1), []);
  return { appSettings, loadError, desktopError, setAppSettingsState, setDesktopError, acceptSnapshot, retryLoad };
}

function useSettingsRestart({ busy, onError, clearError, setBusy }: AppSettingsDeps) {
  const restart = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    clearError();
    try { await restartApplication(); }
    catch (caught) { onError(caught as CommandError); }
    finally { setBusy(false); }
  }, [busy, clearError, onError, setBusy]);
  return { restart };
}

/** Owns the complete app preference snapshot and its one native save path. */
export function useAppSettings(deps: AppSettingsDeps, revision: number) {
  const { busy, onError, clearError, setBusy } = deps;
  const loaded = useLoadedAppSettings(onError, revision);
  const { appSettings, setAppSettingsState, setDesktopError } = loaded;
  const recovery = useSettingsRestart(deps);
  const saving = useRef(false);
  const saveAppSettings = useCallback(async (next: AppSettings) => {
    if (busy || saving.current) return false;
    saving.current = true;
    setBusy(true);
    clearError();
    try {
      const saved = await setAppSettings(next);
      applyLanguagePreference(saved.language);
      setAppSettingsState(saved);
      setDesktopError(null);
      return true;
    } catch (caught) {
      setDesktopError(caught);
      onError(caught as CommandError);
      return false;
    } finally {
      saving.current = false;
      setBusy(false);
    }
  }, [busy, clearError, onError, setBusy, setAppSettingsState, setDesktopError]);
  const saveSettingsPatch = useCallback((patch: Partial<AppSettings>) =>
    appSettings ? saveAppSettings({ ...appSettings, ...patch }) : Promise.resolve(false),
  [appSettings, saveAppSettings]);
  const pin = appSettings ? {
    active: appSettings.alwaysOnTop,
    onToggle: () => saveSettingsPatch({ alwaysOnTop: !appSettings.alwaysOnTop }),
  } : null;
  const toggleUsageExpanded = useCallback((profileId: string) => {
    if (!appSettings) return;
    const expandedUsageIds = appSettings.expandedUsageIds.includes(profileId)
      ? appSettings.expandedUsageIds.filter((id) => id !== profileId)
      : [...appSettings.expandedUsageIds, profileId];
    void saveSettingsPatch({ expandedUsageIds });
  }, [appSettings, saveSettingsPatch]);
  return {
    appSettings, loadError: loaded.loadError, desktopError: loaded.desktopError, retryLoad: loaded.retryLoad,
    ...recovery, saveAppSettings, saveSettingsPatch, pin, toggleUsageExpanded,
  };
}
