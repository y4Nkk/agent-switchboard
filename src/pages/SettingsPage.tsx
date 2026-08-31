import type { AppSettings, UpdateCheck } from "../api/client";
import { AppSettingsForm } from "../components/AppSettingsForm";
import { UpdateSection } from "../components/UpdateSection";

interface SettingsPageProps {
  settings: AppSettings | null;
  busy: boolean;
  /** Saves one field of the currently loaded settings. */
  onPatch: (patch: Partial<AppSettings>) => void;
  /** Latest manual update check; null until the first check runs. */
  updateCheck: UpdateCheck | null;
  onCheckUpdate: () => void;
}

/** Application-runtime settings. Separate from the client common
 * configuration contract. */
export function SettingsPage({
  settings,
  busy,
  onPatch,
  updateCheck,
  onCheckUpdate,
}: SettingsPageProps) {
  return (
    <section className="asb-panel" aria-label="设置">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">设置</h2>
      </div>
      {settings ? (
        <AppSettingsForm
          settings={settings}
          busy={busy}
          onCloseBehaviorChange={(closeBehavior) => onPatch({ closeBehavior })}
          onThemeChange={(theme) => onPatch({ theme })}
          onMotionChange={(motion) => onPatch({ motion })}
          onAlwaysOnTopChange={(alwaysOnTop) => onPatch({ alwaysOnTop })}
          onHardwareAccelerationChange={(hardwareAcceleration) => onPatch({ hardwareAcceleration })}
        />
      ) : (
        <p className="asb-empty">加载中</p>
      )}
      <div className="asb-app-settings">
        <UpdateSection result={updateCheck} busy={busy} onCheck={onCheckUpdate} />
      </div>
    </section>
  );
}
