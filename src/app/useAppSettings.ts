import { useCallback, useEffect, useState } from "react";
import { getAppSettings, setAppSettings, type AppSettings, type CommandError } from "../api/client";
import { quotedFontFamily } from "../lib/font-family";

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
  /** Why the initial load failed; null while loading or after success. */
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    void getAppSettings()
      .then((settings) => {
        setAppSettingsState(settings);
        setLoadError(null);
      })
      .catch((caught) => {
        onError(caught as CommandError);
        setLoadError((caught as CommandError).message);
      });
  }, [onError, reload]);

  const retryLoad = useCallback(() => setReload((count) => count + 1), []);

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
    // The user font leads the display and interface stacks in tokens.css;
    // removing the override falls back to the bundled default there.
    if (appSettings) {
      root.style.setProperty("--asb-font-user", quotedFontFamily(appSettings.interfaceFont));
    } else {
      root.style.removeProperty("--asb-font-user");
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

  return { appSettings, loadError, retryLoad, saveAppSettings, saveSettingsPatch, pin };
}
