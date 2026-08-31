import type {
  AppSettings,
  CloseBehavior,
  MotionPreference,
  ThemePreference,
} from "../api/client";
import { Checkbox } from "./Checkbox";

interface Props {
  settings: AppSettings;
  busy: boolean;
  onCloseBehaviorChange: (value: CloseBehavior) => void;
  onThemeChange: (value: ThemePreference) => void;
  onMotionChange: (value: MotionPreference) => void;
  onAlwaysOnTopChange: (value: boolean) => void;
  onHardwareAccelerationChange: (value: boolean) => void;
}

type SegmentOption<T extends string> = { value: T; label: string; detail: string };

const CLOSE_OPTIONS: SegmentOption<CloseBehavior>[] = [
  {
    value: "hideToTray",
    label: "最小化到托盘",
    detail: "关闭窗口后应用继续在系统托盘运行",
  },
  {
    value: "exit",
    label: "退出应用",
    detail: "关闭窗口时结束应用进程",
  },
];

const THEME_OPTIONS: SegmentOption<ThemePreference>[] = [
  { value: "system", label: "跟随系统", detail: "使用 Windows 当前的浅色或深色外观" },
  { value: "light", label: "浅色", detail: "始终使用浅色外观" },
  { value: "dark", label: "深色", detail: "始终使用深色外观" },
];

const MOTION_OPTIONS: SegmentOption<MotionPreference>[] = [
  { value: "system", label: "跟随系统", detail: "遵循 Windows 的减少动态效果偏好" },
  { value: "reduce", label: "减少动态效果", detail: "停止界面动画与强调性动态效果" },
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
        <span className="asb-app-setting-detail">
          {options.find((option) => option.value === value)!.detail}
        </span>
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
  onAlwaysOnTopChange,
  onHardwareAccelerationChange,
}: Props) {
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
          <Checkbox
            label="窗口始终置顶"
            checked={settings.alwaysOnTop}
            disabled={busy}
            onChange={onAlwaysOnTopChange}
          />
          <span className="asb-app-setting-detail">保持主窗口位于其他窗口前方</span>
        </div>
      </section>
      <section className="asb-app-settings-group" aria-labelledby="performance-settings">
        <h3 id="performance-settings" className="asb-toggle-group-title">
          性能
        </h3>
        <div className="asb-app-setting-row">
          <Checkbox
            label="启用硬件加速"
            checked={settings.hardwareAcceleration}
            disabled={busy}
            onChange={onHardwareAccelerationChange}
          />
          <span className="asb-app-setting-detail">使用 GPU 渲染界面；完整重启应用后生效</span>
        </div>
      </section>
    </div>
  );
}
