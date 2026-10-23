import { useRef } from "react";
import type { AppSettings } from "../api/client";
import { useI18n } from "../i18n";
import { AppSegmentSetting, AppSettingRow } from "./AppPreferenceControls";
import { AppPreviewSetting, CloseBehaviorPreview, LogLevelPreview, MotionPreview, StartupPagePreview } from "./AppPreviewCards";
import { WindowScene } from "./preference-preview/WindowScene";
import { Button } from "./Button";
import { FontPicker } from "./FontPicker";
import { GlobalShortcutSetting } from "./GlobalShortcutSetting";
import { Switch } from "./Switch";

interface Props {
  settings: AppSettings;
  busy: boolean;
  desktopError: string | null;
  onPatch: (patch: Partial<AppSettings>) => Promise<boolean>;
  onRestart: () => void;
}

function AppearanceSettings({ settings, busy, onPatch }: Props) {
  const { t } = useI18n();
  return (
    <section className="asb-app-settings-group" aria-labelledby="appearance-settings">
      <h3 id="appearance-settings" className="asb-section-title">{t("settings.group.appearance")}</h3>
      <AppSegmentSetting<AppSettings["language"]> label={t("settings.language")} detail={t("settings.language.detail")}
        value={settings.language} busy={busy}
        options={[{ value: "system", label: t("settings.language.system") }, { value: "zh-CN", label: t("settings.language.zh") }, { value: "en-US", label: t("settings.language.en") }]}
        onChange={(language) => void onPatch({ language })} />
      <AppPreviewSetting<AppSettings["theme"]> label={t("settings.appearance.theme")} value={settings.theme} busy={busy}
        options={[
          { value: "light", label: t("settings.appearance.theme.light"), description: t("settings.preview.theme.light") },
          { value: "system", label: t("settings.appearance.theme.system"), description: t("settings.preview.theme.system") },
          { value: "dark", label: t("settings.appearance.theme.dark"), description: t("settings.preview.theme.dark") },
        ]}
        renderPreview={(theme) => <WindowScene theme={theme} />}
        onChange={(theme) => void onPatch({ theme })} />
      <AppSettingRow label={t("settings.appearance.font")}>
        <FontPicker value={settings.interfaceFont} busy={busy} onChange={(interfaceFont) => void onPatch({ interfaceFont })} />
      </AppSettingRow>
      <AppPreviewSetting<AppSettings["interfaceScale"]> label={t("settings.appearance.scale")}
        value={settings.interfaceScale} busy={busy}
        options={[
          { value: 90, label: "90%", description: t("settings.preview.scale.90") },
          { value: 100, label: "100%", description: t("settings.preview.scale.100") },
          { value: 110, label: "110%", description: t("settings.preview.scale.110") },
          { value: 125, label: "125%", description: t("settings.preview.scale.125") },
        ]}
        renderPreview={(scale) => <WindowScene theme="light" scale={scale} />}
        onChange={(interfaceScale) => void onPatch({ interfaceScale })} />
      <AppPreviewSetting<AppSettings["motion"]> label={t("settings.appearance.motion")} value={settings.motion} busy={busy}
        options={[
          { value: "system", label: t("settings.appearance.motion.system"), description: t("settings.preview.motion.system") },
          { value: "reduce", label: t("settings.appearance.motion.reduce"), description: t("settings.preview.motion.reduce") },
        ]}
        renderPreview={(mode) => <MotionPreview mode={mode} />}
        onChange={(motion) => void onPatch({ motion })} />
    </section>
  );
}

function WindowSettings({ settings, busy, onPatch }: Props) {
  const { t } = useI18n();
  return (
    <section className="asb-app-settings-group" aria-labelledby="window-tray-settings">
      <h3 id="window-tray-settings" className="asb-section-title">{t("settings.group.window")}</h3>
      <AppPreviewSetting<AppSettings["closeBehavior"]> label={t("settings.window.closeAction")} value={settings.closeBehavior} busy={busy}
        options={[
          { value: "hideToTray", label: t("settings.window.close.hideToTray"), description: t("settings.preview.close.hideToTray") },
          { value: "exit", label: t("settings.window.close.exit"), description: t("settings.preview.close.exit") },
        ]}
        renderPreview={(behavior) => <CloseBehaviorPreview behavior={behavior} />}
        onChange={(closeBehavior) => void onPatch({ closeBehavior })} />
      <AppSettingRow label={t("settings.window.alwaysOnTop")}>
        <Switch label={t("settings.window.alwaysOnTop")} checked={settings.alwaysOnTop} disabled={busy}
          onChange={(alwaysOnTop) => void onPatch({ alwaysOnTop })} />
      </AppSettingRow>
      <AppSettingRow label={t("settings.window.launchAtLogin")}>
        <Switch label={t("settings.window.launchAtLogin")} checked={settings.launchAtLogin} disabled={busy}
          onChange={(launchAtLogin) => void onPatch({ launchAtLogin })} />
      </AppSettingRow>
      <AppSettingRow label={t("settings.window.startMinimized")}>
        <Switch label={t("settings.window.startMinimized")} checked={settings.startMinimized} disabled={busy}
          onChange={(startMinimized) => void onPatch({ startMinimized })} />
      </AppSettingRow>
      <AppPreviewSetting<AppSettings["startupPage"]> label={t("settings.window.startupPage")} detail={t("settings.window.startupPage.detail")}
        value={settings.startupPage} busy={busy}
        options={[
          { value: "providers", label: t("settings.window.startupPage.providers"), description: t("settings.preview.startup.providers") },
          { value: "lastVisited", label: t("settings.window.startupPage.lastVisited"), description: t("settings.preview.startup.lastVisited") },
        ]}
        renderPreview={(page) => <StartupPagePreview page={page} />}
        onChange={(startupPage) => void onPatch({ startupPage })} />
      <GlobalShortcutSetting value={settings.globalShortcut} busy={busy}
        onChange={(globalShortcut) => onPatch({ globalShortcut })} />
    </section>
  );
}

function PerformanceSettings({ settings, busy, onPatch, onRestart }: Props) {
  const { t } = useI18n();
  const initialAcceleration = useRef(settings.hardwareAcceleration);
  if (!/Windows/.test(navigator.userAgent)) return null;
  return (
    <section className="asb-app-settings-group" aria-labelledby="performance-settings">
      <h3 id="performance-settings" className="asb-section-title">{t("settings.group.performance")}</h3>
      <AppSettingRow label={t("settings.performance.hardwareAcceleration")}>
        <Switch label={t("settings.performance.hardwareAcceleration")} checked={settings.hardwareAcceleration} disabled={busy}
          onChange={(hardwareAcceleration) => void onPatch({ hardwareAcceleration })} />
      </AppSettingRow>
      {initialAcceleration.current !== settings.hardwareAcceleration && (
        <AppSettingRow label={t("settings.performance.restartRequired")}>
          <Button variant="secondary" disabled={busy} onClick={onRestart}>{t("settings.performance.restart")}</Button>
        </AppSettingRow>
      )}
    </section>
  );
}

/** Application preferences use the single native save transaction. */
export function AppSettingsForm(props: Props) {
  const { settings, busy, onPatch } = props;
  const { t } = useI18n();
  return (
    <div className="asb-app-settings">
      {props.desktopError && <p className="asb-field-error" role="alert">{props.desktopError}</p>}
      <AppearanceSettings {...props} />
      <WindowSettings {...props} />
      <section className="asb-app-settings-group" aria-labelledby="log-settings">
        <h3 id="log-settings" className="asb-section-title">{t("settings.group.log")}</h3>
        <AppPreviewSetting<AppSettings["runtimeLogLevel"]> label={t("settings.log.level")} detail={t("settings.log.level.detail")}
          value={settings.runtimeLogLevel} busy={busy}
          options={[
            { value: "debug", label: t("settings.log.level.debug"), description: t("settings.preview.log.debug") },
            { value: "info", label: t("settings.log.level.info"), description: t("settings.preview.log.info") },
            { value: "warn", label: t("settings.log.level.warn"), description: t("settings.preview.log.warn") },
            { value: "error", label: t("settings.log.level.error"), description: t("settings.preview.log.error") },
            { value: "silent", label: t("settings.log.level.silent"), description: t("settings.preview.log.silent") },
          ]}
          renderPreview={(level) => <LogLevelPreview level={level} />}
          onChange={(runtimeLogLevel) => void onPatch({ runtimeLogLevel })} />
      </section>
      <PerformanceSettings {...props} />
    </div>
  );
}
