import { useCallback, useEffect, useState } from "react";
import { getAppSettings, setAppSettings, type AppSettings, type CommandError } from "../api/client";

interface AppSettingsDeps {
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
}

/**
 * Application-runtime settings (window behavior, appearance) and their
 * live-appearance application. Deliberately separate from the Codex / Claude
 * common configuration contract.
 */
export function useAppSettings({ busy, onError, clearError, setBusy }: AppSettingsDeps) {
  const [appSettings, setAppSettingsState] = useState<AppSettings | null>(null);

  useEffect(() => {
    void getAppSettings()
      .then(setAppSettingsState)
      .catch((caught) => onError(caught as CommandError));
  }, [onError]);

  useEffect(() => {
    const root = document.documentElement;
    if (!appSettings || appSettings.theme === "system") {
      delete root.dataset.theme;
    } else {
      root.dataset.theme = appSettings.theme;
    }
    if (!appSettings || appSettings.motion === "system") {
      delete root.dataset.motion;
    } else {
      root.dataset.motion = appSettings.motion;
    }
  }, [appSettings]);

  const saveAppSettings = useCallback(
    async (next: AppSettings) => {
      if (busy) return;
      setBusy(true);
      clearError();
      try {
        setAppSettingsState(await setAppSettings(next));
      } catch (caught) {
        onError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, setBusy],
  );

  const saveSettingsPatch = useCallback(
    (patch: Partial<AppSettings>) => {
      if (appSettings) void saveAppSettings({ ...appSettings, ...patch });
    },
    [appSettings, saveAppSettings],
  );

  /** The title-bar always-on-top toggle; null until settings load. */
  const pin = appSettings
    ? {
        active: appSettings.alwaysOnTop,
        onToggle: () => saveSettingsPatch({ alwaysOnTop: !appSettings.alwaysOnTop }),
      }
    : null;

  return { appSettings, saveAppSettings, saveSettingsPatch, pin };
}
