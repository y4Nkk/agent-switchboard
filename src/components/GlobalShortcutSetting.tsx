import { globalShortcutLabel } from "../lib/global-shortcut";
import { useI18n } from "../i18n";
import { SettingsRow } from "./SettingsLayout";
import { Button } from "./Button";
import { useGlobalShortcutRecorder } from "./useGlobalShortcutRecorder";

export function GlobalShortcutSetting({ value, busy, onChange }: {
  value: string;
  busy: boolean;
  onChange: (value: string) => Promise<boolean>;
}) {
  const recorder = useGlobalShortcutRecorder(onChange);
  const { t } = useI18n();
  const { recording, preparing, error } = recorder;
  return (
    <SettingsRow label={t("settings.shortcut.label")} description={t("settings.shortcut.detail")}>
      <div className="asb-shortcut-setting">
        <div className="asb-shortcut-controls">
          <Button ref={recorder.captureButton} variant="secondary" disabled={busy} aria-pressed={recording} aria-busy={preparing}
            aria-label={recording ? t("settings.shortcut.recordAriaRecording") : t("settings.shortcut.recordAria", { current: globalShortcutLabel(value) })}
            onClick={() => { if (!recording && !preparing) void recorder.start(); }} onKeyDown={recorder.capture}
            onBlur={() => { if (recording || preparing) recorder.finish(); }}>
            {preparing ? t("settings.shortcut.preparing") : recording ? t("settings.shortcut.recording") : globalShortcutLabel(value)}
          </Button>
          {recording ? <Button variant="secondary" onClick={recorder.finish}>{t("confirm.cancel")}</Button>
            : value && <Button variant="secondary" disabled={busy || preparing} onClick={() => void onChange("")}>{t("settings.shortcut.clear")}</Button>}
        </div>
        {error && <p className="asb-field-error" role="alert">{error}</p>}
      </div>
    </SettingsRow>
  );
}
