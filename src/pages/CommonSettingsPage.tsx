import { useState } from "react";
import type {
  AppKind,
  CommonValue,
  ConfigFileStatus,
  GlobalPromptDocument,
} from "../api/client";
import type { CommonSettingsEditorState } from "../app/useCommonSettings";
import { ClientLogo } from "../components/ClientLogo";
import { CodePreview } from "../components/CodePreview";
import { GeneralSettingsForm } from "../components/GeneralSettingsForm";
import { GlobalPromptManager } from "../components/GlobalPromptManager";
import { Button } from "../components/Button";
import { OfficialSettingsDirectory } from "../components/OfficialSettingsDirectory";
import { Tooltip } from "../components/Tooltip";
import { clientName } from "../lib/client-name";

interface CommonSettingsPageProps {
  app: AppKind;
  onSelectApp: (app: AppKind) => void;
  editorState: CommonSettingsEditorState;
  busy: boolean;
  /** Current real-client state. General settings never write it directly. */
  configStatus: ConfigFileStatus | undefined;
  /** Whether this client currently has an applied supplier; when true a
   * save only takes effect after the supplier is re-applied. */
  hasActiveProvider: boolean;
  onValueChange: (app: AppKind, key: string, value: CommonValue) => void;
  onResetGroup: (app: AppKind, group: string | null) => void;
  onSave: (app: AppKind) => void;
  onRetryLoad: (app: AppKind) => void;
  onPreview: (app: AppKind) => void;
  promptDocument: GlobalPromptDocument | undefined;
  promptDraft: string;
  promptDirty: boolean;
  onPromptDraftChange: (content: string) => void;
  onSavePrompt: () => void;
  onDiscardPrompt: () => void;
  onReloadPrompt: () => void;
}

function SettingsEditor({
  state,
  busy,
  configStatus,
  hasActiveProvider,
  onValueChange,
  onResetGroup,
  onSave,
  onRetryLoad,
  onPreview,
}: {
  state: CommonSettingsEditorState;
  busy: boolean;
  configStatus: ConfigFileStatus | undefined;
  hasActiveProvider: boolean;
  onValueChange: (key: string, value: CommonValue) => void;
  onResetGroup: (group: string | null) => void;
  onSave: () => void;
  onRetryLoad: () => void;
  onPreview: () => void;
}) {
  // One control owns the preview's whole lifecycle (ProbePanel idiom):
  // first click renders the draft, later clicks fold the shown fragment
  // away and back out. A draft edit clears the fragment in the hook, so a
  // stale open flag can never display content on its own.
  const [previewOpen, setPreviewOpen] = useState(false);
  const previewVisible = previewOpen && state.preview !== undefined;

  if (state.phase === "idle" || state.phase === "loading") {
    return <p className="asb-empty">正在读取通用设置</p>;
  }
  if (state.phase === "loadError" || !state.editor || !state.draft) {
    return (
      <div className="asb-empty" role="alert">
        <p>无法读取通用设置：{state.error?.message ?? "本地应用数据不可用"}</p>
        <Button variant="secondary" disabled={busy} onClick={onRetryLoad}>
          重新读取
        </Button>
      </div>
    );
  }

  const canSave = state.phase === "dirty" || state.phase === "saveError";
  const realConfigStatus = (() => {
    switch (configStatus?.matchStatus.kind) {
      case "matchesProfile":
        return `已应用：真实配置与「${configStatus.matchStatus.profileName}」一致`;
      case "externallyModified":
        return "真实配置已被外部修改，请前往供应商页重新应用";
      case "profileChanged":
        return `供应商「${configStatus.matchStatus.profileName}」已更新，请重新应用`;
      case "restoredBackup":
        return "真实配置已恢复备份，请前往供应商页重新应用";
      case "unmanaged":
        return "真实配置尚未由供应商应用管理";
      case "unknown":
      case undefined:
        return null;
    }
  })();
  const actionStatus = (() => {
    if (state.phase === "dirty") return { message: "有未保存修改", error: false };
    if (state.phase === "savedPendingReapply") {
      return {
        message: hasActiveProvider
          ? "已保存，重新应用当前供应商后生效"
          : "已保存，请在供应商页选择并启用供应商后生效",
        error: false,
      };
    }
    if (realConfigStatus) {
      return {
        message: realConfigStatus,
        error: configStatus?.matchStatus.kind === "externallyModified",
      };
    }
    if (state.phase === "clean") return { message: "已保存到应用数据", error: false };
    return null;
  })();

  const previewLabel = state.previewing
    ? "正在生成预览"
    : previewVisible
      ? "收起通用配置预览"
      : state.preview
        ? "展开通用配置预览"
        : "查看通用配置预览";

  return (
    <>
      <GeneralSettingsForm
        specs={state.editor.specs}
        groups={state.editor.groups}
        values={state.draft}
        busy={busy}
        onChange={onValueChange}
        onResetGroup={onResetGroup}
      />
      <div className="asb-general-settings-actions">
        <Button
          variant="secondary"
          disabled={busy}
          onClick={() => onResetGroup(null)}
        >
          全部恢复默认值
        </Button>
        <div className="asb-general-settings-actions-main">
          {actionStatus ? (
            <span
              className={actionStatus.error ? "asb-field-error" : "asb-field-help"}
              role={actionStatus.error ? "alert" : "status"}
            >
              {actionStatus.message}
            </span>
          ) : null}
          <Button
            variant="primary"
            disabled={busy || !canSave}
            onClick={onSave}
          >
            {state.phase === "saving" ? "正在保存" : "保存通用设置"}
          </Button>
        </div>
      </div>
      {state.phase === "saveError" ? (
        <p className="asb-field-error" role="alert">
          保存失败，修改已保留：{state.error?.message ?? "请重试"}
        </p>
      ) : null}
      <div className="asb-settings-preview">
        <div className="asb-form-actions">
          <Button
            variant="secondary"
            aria-expanded={previewVisible}
            aria-controls="general-settings-preview"
            disabled={busy || state.previewing}
            onClick={() => {
              if (previewVisible) {
                setPreviewOpen(false);
                return;
              }
              setPreviewOpen(true);
              // A cached fragment is never stale (a draft edit clears it in
              // the hook), so only the first render needs the backend.
              if (!state.preview) onPreview();
            }}
          >
            {previewLabel}
          </Button>
        </div>
        {previewVisible && state.preview ? (
          <div id="general-settings-preview">
            <CodePreview target={state.preview.target} content={state.preview.content} />
          </div>
        ) : null}
        {state.previewError ? (
          <p className="asb-field-error" role="alert">
            无法生成通用配置预览：{state.previewError.message}
          </p>
        ) : null}
      </div>
    </>
  );
}

/** Application-wide general parameters and user-global prompts. Saving
 * values here is deliberately separate from a supplier activation and the
 * actual client file produced by it. */
export function CommonSettingsPage({
  app,
  onSelectApp,
  editorState,
  busy,
  configStatus,
  hasActiveProvider,
  onValueChange,
  onResetGroup,
  onSave,
  onRetryLoad,
  onPreview,
  promptDocument,
  promptDraft,
  promptDirty,
  onPromptDraftChange,
  onSavePrompt,
  onDiscardPrompt,
  onReloadPrompt,
}: CommonSettingsPageProps) {
  const [section, setSection] = useState<"settings" | "directory" | "prompts">("settings");
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
      <div className="asb-settings-main-tabs" role="tablist" aria-label="通用设置内容">
        <button
          type="button"
          role="tab"
          aria-selected={section === "settings"}
          className={`asb-settings-main-tab${section === "settings" ? " is-on" : ""}`}
          onClick={() => setSection("settings")}
        >
          基础参数
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={section === "directory"}
          className={`asb-settings-main-tab${section === "directory" ? " is-on" : ""}`}
          onClick={() => setSection("directory")}
        >
          官方设置目录
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={section === "prompts"}
          className={`asb-settings-main-tab${section === "prompts" ? " is-on" : ""}`}
          onClick={() => setSection("prompts")}
        >
          全局指令
        </button>
      </div>
      {section === "settings" ? (
        <SettingsEditor
          state={editorState}
          configStatus={configStatus}
          hasActiveProvider={hasActiveProvider}
          busy={busy}
          onValueChange={(key, value) => onValueChange(app, key, value)}
          onResetGroup={(group) => onResetGroup(app, group)}
          onSave={() => onSave(app)}
          onRetryLoad={() => onRetryLoad(app)}
          onPreview={() => onPreview(app)}
        />
      ) : section === "directory" ? (
        <OfficialSettingsDirectory app={app} entries={editorState.editor?.directory ?? []} />
      ) : (
        <GlobalPromptManager
          document={promptDocument}
          draft={promptDraft}
          dirty={promptDirty}
          busy={busy}
          onChange={onPromptDraftChange}
          onSave={onSavePrompt}
          onDiscard={onDiscardPrompt}
          onReload={onReloadPrompt}
        />
      )}
    </section>
  );
}
