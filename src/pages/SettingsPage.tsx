import type { AppSettings, UpdateCheck } from "../api/client";
import { AppSettingsForm } from "../components/AppSettingsForm";
import { UpdateSection } from "../components/UpdateSection";

interface SettingsPageProps {
  settings: AppSettings | null;
  /** Why settings could not load; null while loading or after success. */
  loadError: string | null;
  /** Re-runs the settings load after a failure. */
  onRetryLoad: () => void;
  busy: boolean;
  /** Saves one field of the currently loaded settings. */
  onPatch: (patch: Partial<AppSettings>) => void;
  /** Restarts the desktop process on the user's explicit request. */
  onRestart: () => void;
  /** Latest manual update check; null until the first check runs. */
  updateCheck: UpdateCheck | null;
  /** A startup or user-triggered release lookup is currently in flight. */
  updateChecking: boolean;
  onCheckUpdate: () => void;
}

/** Application-runtime settings. Separate from the client common
 * configuration contract. */
export function SettingsPage({
  settings,
  loadError,
  onRetryLoad,
  busy,
  onPatch,
  onRestart,
  updateCheck,
  updateChecking,
  onCheckUpdate,
}: SettingsPageProps) {
  return (
    <section className="asb-panel" aria-label="设置">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">设置</h2>
      </div>
      <div className="asb-app-settings">
        {settings ? (
          <AppSettingsForm
            settings={settings}
            busy={busy}
            onCloseBehaviorChange={(closeBehavior) => onPatch({ closeBehavior })}
            onThemeChange={(theme) => onPatch({ theme })}
            onMotionChange={(motion) => onPatch({ motion })}
            onInterfaceFontChange={(interfaceFont) => onPatch({ interfaceFont })}
            onAlwaysOnTopChange={(alwaysOnTop) => onPatch({ alwaysOnTop })}
            onLaunchAtLoginChange={(launchAtLogin) => onPatch({ launchAtLogin })}
            onHardwareAccelerationChange={(hardwareAcceleration) =>
              onPatch({ hardwareAcceleration })
            }
            onRestart={onRestart}
          />
        ) : loadError ? (
          <div className="asb-app-setting-row" role="alert">
            <div className="asb-app-setting-copy">
              <span className="asb-checkbox-label">设置加载失败：{loadError}</span>
              <span className="asb-app-setting-detail">
                读取失败期间，外观与关闭行为使用默认值
              </span>
            </div>
            <button
              type="button"
              className="asb-btn-secondary"
              disabled={busy}
              onClick={onRetryLoad}
            >
              重试
            </button>
          </div>
        ) : (
          <p className="asb-empty">加载中</p>
        )}
        <UpdateSection result={updateCheck} busy={busy || updateChecking} onCheck={onCheckUpdate} />
      </div>
    </section>
  );
}
