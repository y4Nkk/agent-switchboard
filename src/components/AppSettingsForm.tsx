import { useState } from "react";
import type {
  AppSettings,
  CloseBehavior,
  MotionPreference,
  ThemePreference,
} from "../api/client";
import { Button } from "./Button";
import { FontPicker } from "./FontPicker";
import { Switch } from "./Switch";

interface Props {
  settings: AppSettings;
  busy: boolean;
  onCloseBehaviorChange: (value: CloseBehavior) => void;
  onThemeChange: (value: ThemePreference) => void;
  onMotionChange: (value: MotionPreference) => void;
  onInterfaceFontChange: (value: string) => void;
  onAlwaysOnTopChange: (value: boolean) => void;
  onLaunchAtLoginChange: (value: boolean) => void;
  onHardwareAccelerationChange: (value: boolean) => void;
  onRestart: () => void;
}

type SegmentOption<T extends string> = { value: T; label: string };

const CLOSE_OPTIONS: SegmentOption<CloseBehavior>[] = [
  { value: "hideToTray", label: "最小化到托盘" },
  { value: "exit", label: "退出应用" },
];

const THEME_OPTIONS: SegmentOption<ThemePreference>[] = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
];

const MOTION_OPTIONS: SegmentOption<MotionPreference>[] = [
  { value: "system", label: "跟随系统" },
  { value: "reduce", label: "减少动态效果" },
];

function SegmentSetting<T extends string>({
  label,
  value,
  options,
  busy,
  onChange,
}: {
  label: string;
  value: T;
  options: SegmentOption<T>[];
  busy: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div className="asb-app-setting-row">
      <div className="asb-app-setting-copy">
        <span className="asb-checkbox-label">{label}</span>
      </div>
      <div className="asb-segments" role="radiogroup" aria-label={label}>
        {options.map((option) => {
          const active = value === option.value;
          return (
            <label className={`asb-seg-opt${active ? " is-active" : ""}`} key={option.value}>
              <input
                type="radio"
                name={label}
                value={option.value}
                checked={active}
                disabled={busy}
                onChange={() => onChange(option.value)}
              />
              {option.label}
            </label>
          );
        })}
      </div>
    </div>
  );
}

/** Application-runtime preferences. Unlike GeneralSettingsForm, these values
    describe Agent Switchboard itself and never write client configuration. */
export function AppSettingsForm({
  settings,
  busy,
  onCloseBehaviorChange,
  onThemeChange,
  onMotionChange,
  onInterfaceFontChange,
  onAlwaysOnTopChange,
  onLaunchAtLoginChange,
  onHardwareAccelerationChange,
  onRestart,
}: Props) {
  const [hardwareAccelerationRestartRequired, setHardwareAccelerationRestartRequired] = useState(false);

  const handleHardwareAccelerationChange = (value: boolean) => {
    onHardwareAccelerationChange(value);
    setHardwareAccelerationRestartRequired(true);
  };

  return (
    <div className="asb-app-settings">
      <section className="asb-app-settings-group" aria-labelledby="appearance-settings">
        <h3 id="appearance-settings" className="asb-toggle-group-title">
          外观
        </h3>
        <SegmentSetting
          label="界面主题"
          value={settings.theme}
          options={THEME_OPTIONS}
          busy={busy}
          onChange={onThemeChange}
        />
        <div className="asb-app-setting-row">
          <div className="asb-app-setting-copy">
            <span className="asb-checkbox-label">界面字体</span>
          </div>
          <FontPicker value={settings.interfaceFont} busy={busy} onChange={onInterfaceFontChange} />
        </div>
        <SegmentSetting
          label="动态效果"
          value={settings.motion}
          options={MOTION_OPTIONS}
          busy={busy}
          onChange={onMotionChange}
        />
      </section>
      <section className="asb-app-settings-group" aria-labelledby="window-tray-settings">
        <h3 id="window-tray-settings" className="asb-toggle-group-title">
          窗口与托盘
        </h3>
        <SegmentSetting
          label="点击关闭按钮时"
          value={settings.closeBehavior}
          options={CLOSE_OPTIONS}
          busy={busy}
          onChange={onCloseBehaviorChange}
        />
        <div className="asb-app-setting-row">
          <div className="asb-app-setting-copy">
            <span className="asb-checkbox-label">窗口始终置顶</span>
          </div>
          <Switch
            label="窗口始终置顶"
            checked={settings.alwaysOnTop}
            disabled={busy}
            onChange={onAlwaysOnTopChange}
          />
        </div>
        <div className="asb-app-setting-row">
          <div className="asb-app-setting-copy">
            <span className="asb-checkbox-label">开机自动启动</span>
          </div>
          <Switch
            label="开机自动启动"
            checked={settings.launchAtLogin}
            disabled={busy}
            onChange={onLaunchAtLoginChange}
          />
        </div>
      </section>
      <section className="asb-app-settings-group" aria-labelledby="performance-settings">
        <h3 id="performance-settings" className="asb-toggle-group-title">
          性能
        </h3>
        <div className="asb-app-setting-row">
          <div className="asb-app-setting-copy">
            <span className="asb-checkbox-label">启用硬件加速</span>
          </div>
          <Switch
            label="启用硬件加速"
            checked={settings.hardwareAcceleration}
            disabled={busy}
            onChange={handleHardwareAccelerationChange}
          />
        </div>
        {hardwareAccelerationRestartRequired && (
          <div className="asb-app-setting-row">
            <div className="asb-app-setting-copy">
              <span className="asb-checkbox-label">重启以应用硬件加速</span>
            </div>
            <Button variant="secondary" disabled={busy} onClick={onRestart}>
              重启应用
            </Button>
          </div>
        )}
      </section>
    </div>
  );
}
