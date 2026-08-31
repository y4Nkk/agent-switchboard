import type {
  AppKind,
  CommonChoicesState,
  FilePreview,
  ToggleState,
} from "../api/client";
import { ClientLogo } from "../components/ClientLogo";
import { CodePreview } from "../components/CodePreview";
import { GeneralSettingsForm } from "../components/GeneralSettingsForm";
import { Tooltip } from "../components/Tooltip";
import { clientName } from "../lib/client-name";

interface CommonSettingsPageProps {
  app: AppKind;
  onSelectApp: (app: AppKind) => void;
  toggles: ToggleState[] | undefined;
  choices: CommonChoicesState | undefined;
  commonPreview: FilePreview | null | undefined;
  busy: boolean;
  /** One control = one config line, written through the safe transaction. */
  onApplyLine: (app: AppKind, key: string, value: boolean | string | null) => void;
}

/** General settings: per-client overlay controls plus the live candidate
 * file preview. */
export function CommonSettingsPage({
  app,
  onSelectApp,
  toggles,
  choices,
  commonPreview,
  busy,
  onApplyLine,
}: CommonSettingsPageProps) {
  return (
    <section className="asb-panel" aria-label="通用设置">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">通用设置</h2>
      </div>
      <div className="asb-tabs" role="tablist" aria-label="客户端">
        {(["codex", "claude"] as const).map((target) => (
          <Tooltip key={target} label={clientName(target)} side="bottom">
            <button
              type="button"
              role="tab"
              aria-selected={app === target}
              aria-label={clientName(target)}
              className={`asb-tab${app === target ? " is-on" : ""}`}
              onClick={() => onSelectApp(target)}
            >
              <ClientLogo app={target} className="asb-tab-logo" />
            </button>
          </Tooltip>
        ))}
      </div>
      <p className="asb-scope-note">
        勾选即在配置文件写入该行，取消勾选即移除；写入前自动备份并原子替换。仅管理用户级配置，项目级配置与命令行参数可能覆盖此处设置。
      </p>
      {toggles && choices ? (
        <GeneralSettingsForm
          app={app}
          toggles={toggles}
          choices={choices.choices}
          groups={choices.groups}
          busy={busy}
          onToggle={(toggle, checked) =>
            onApplyLine(app, toggle.key, checked ? toggle.applied : null)
          }
          onChoiceChange={(choice, value) => onApplyLine(app, choice.key, value)}
        />
      ) : (
        <p className="asb-empty">加载中</p>
      )}
      <div className="asb-settings-preview">
        {commonPreview ? (
          <CodePreview target={commonPreview.preview.target} content={commonPreview.content} />
        ) : (
          <p className="asb-empty">正在生成配置预览</p>
        )}
      </div>
    </section>
  );
}
