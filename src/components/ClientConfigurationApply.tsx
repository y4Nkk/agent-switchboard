import { errorText, uiMessage } from "../i18n/errors";
import type { CommandError } from "../api/client";
import { useMessageState } from "../i18n/use-message-state";
import { useState } from "react";

import {
  commitClientConfigurationApply,
  commitClientConfigurationReset,
  previewClientConfigurationApply,
  previewClientConfigurationReset,
  type AppKind,
  type ClientConfigurationApplyPreview,
  type ClientConfigurationResetKind,
  type CodexSubagentSettings,
  type ConfigFileStatus,
  type NativeConfigurationResetKind,
  type SettingsValues,
} from "../api/client";
import { clientSettingsPayload } from "../app/claude-common-settings";
import type { ClientSettingsEditorState } from "../app/useClientSettings";
import { useI18n } from "../i18n";
import { localizedMessageText } from "../i18n/errors";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import { ConfirmSheet } from "./ConfirmSheet";
import { DiffView } from "./DiffView";
import { PreviewInspector } from "./PreviewInspector";

const CLAUDE_EXTRA_MANIFEST_KEYS = new Set([
  "/env/ASB_CLAUDE_COMMON_KEYS",
  "env.ASB_CLAUDE_COMMON_KEYS",
]);

function visibleResetChanges(
  resetKind: ClientConfigurationResetKind,
  preview: ClientConfigurationApplyPreview,
) {
  const changes = preview.file.preview.changes;
  return resetKind === "clearExtraConfiguration"
    ? changes.filter((change) => !CLAUDE_EXTRA_MANIFEST_KEYS.has(change.key))
    : changes;
}

function NativeResetScope({ app, advanced }: { app: AppKind; advanced: boolean }) {
  const { t } = useI18n();
  return (
    <div className="asb-client-settings-reset-scope">
      <span className="asb-client-settings-reset-scope-label">{t("clientConfig.reset.scopeWillRestore")}</span>
      <ul>
        <li>{t("clientConfig.reset.scopeStandardSettings")}</li>
        {app === "codex" && <li>{t("clientConfig.reset.scopeCodexSubagent")}</li>}
        {advanced && <li>{t("clientConfig.reset.scopeUnmanaged")}</li>}
      </ul>
      <p className="asb-field-help">
        {advanced
          ? t("clientConfig.reset.advancedHelp")
          : t("clientConfig.reset.basicHelp")}
      </p>
    </div>
  );
}

function ExtraResetScope({ extraConfigurationCount }: { extraConfigurationCount: number }) {
  const { t } = useI18n();
  return (
    <div className="asb-client-settings-reset-scope">
      <span className="asb-client-settings-reset-scope-label">{t("clientConfig.reset.scopeWillClear")}</span>
      <p>{t("clientConfig.reset.extraScopeLine", { count: extraConfigurationCount })}</p>
      <p className="asb-field-help">
        {t("clientConfig.reset.extraScopeHelp")}
      </p>
    </div>
  );
}

export function ClientConfigurationResetPreview({
  preview,
  app,
  resetKind,
  extraConfigurationCount,
  busy,
  onConfirm,
  onScopeChange,
  onBack,
}: {
  preview: ClientConfigurationApplyPreview;
  app: AppKind;
  resetKind: ClientConfigurationResetKind;
  extraConfigurationCount: number;
  busy: boolean;
  onConfirm: () => void;
  onScopeChange?: (resetKind: NativeConfigurationResetKind) => void;
  onBack?: () => void;
}) {
  const deep = resetKind === "nativeDefaultsWithUnmanaged";
  const native = resetKind !== "clearExtraConfiguration";
  const changes = visibleResetChanges(resetKind, preview);
  const { t } = useI18n();
  const changeLabel = native
    ? deep ? t("clientConfig.reset.changeFields") : t("clientConfig.reset.changeStandard")
    : t("clientConfig.reset.changeExtra");
  const confirmLabel = !native
    ? t("clientConfig.reset.confirmExtra", { count: extraConfigurationCount })
    : deep ? t("clientConfig.reset.confirmDeep") : t("clientConfig.reset.confirmNative");
  return (
    <div className="asb-client-settings-reset-preview">
      <p className="asb-client-settings-reset-lead">
        {!native
          ? t("clientConfig.reset.extraLead")
          : deep
            ? t("clientConfig.reset.deepLead")
            : t("clientConfig.reset.nativeLead")}
      </p>
      {native ? (
        <>
          {onScopeChange && (
            <div className="asb-client-settings-reset-advanced">
              <Checkbox
                checked={deep}
                disabled={busy}
                label={t("clientConfig.reset.advancedCheckbox")}
                onChange={(checked) =>
                  onScopeChange(checked ? "nativeDefaultsWithUnmanaged" : "nativeDefaults")}
              />
              <p className="asb-field-help">
                {t("clientConfig.reset.advancedCheckboxHelp")}
              </p>
            </div>
          )}
          <NativeResetScope app={app} advanced={deep} />
        </>
      ) : (
        <ExtraResetScope extraConfigurationCount={extraConfigurationCount} />
      )}
      <p className="asb-client-settings-reset-target">
        <span>{t("clientConfig.reset.targetFile")}</span>
        <code>{preview.file.preview.target}</code>
      </p>
      {changes.length > 0 ? (
        <div className="asb-client-settings-reset-changes">
          <DiffView changes={changes} label={changeLabel} />
        </div>
      ) : (
        <p className="asb-field-help">
          {t("clientConfig.reset.noChanges")}
        </p>
      )}
      {preview.file.preview.warnings.map((warning) => (
        <p key={warning.key} className="asb-field-error" role="alert">{localizedMessageText(warning, t)}</p>
      ))}
      <p className="asb-field-help">
        {t("clientConfig.reset.backupHelp")}
      </p>
      <div className="asb-client-settings-reset-actions">
        {onBack && (
          <Button variant="secondary" className="asb-client-settings-reset-back" disabled={busy} onClick={onBack}>
            {t("clientConfig.reset.backToSettings")}
          </Button>
        )}
        <Button variant="danger" disabled={busy} onClick={onConfirm}>
          {confirmLabel}
        </Button>
      </div>
    </div>
  );
}

export interface PendingClientConfiguration {
  preview: ClientConfigurationApplyPreview;
  settings: SettingsValues;
  subagentSettings?: CodexSubagentSettings;
}

export interface PendingClientConfigurationReset {
  preview: ClientConfigurationApplyPreview;
  resetKind: ClientConfigurationResetKind;
}

interface UseClientConfigurationApplyProps {
  app: AppKind;
  busy: boolean;
  editorState: ClientSettingsEditorState;
  subagentDraft?: CodexSubagentSettings;
  configStatus?: ConfigFileStatus;
  hasUnsavedConfigurationDraft: boolean;
  onApplied: () => void;
}

function extraConfigurationCount(extra: Record<string, unknown> | undefined): number {
  if (!extra) return 0;
  const count = (value: unknown): number => {
    if (!value || typeof value !== "object" || Array.isArray(value)) return 1;
    const entries = Object.values(value);
    return entries.length === 0 ? 0 : entries.reduce<number>((total, child) => total + count(child), 0);
  };
  return Object.values(extra).reduce<number>((total, value) => total + count(value), 0);
}

type ResetState<T> = Record<ClientConfigurationResetKind, T>;

function emptyResetState<T>(value: T): ResetState<T> {
  return {
    nativeDefaults: value,
    nativeDefaultsWithUnmanaged: value,
    clearExtraConfiguration: value,
  };
}

interface UseClientConfigurationResetsProps {
  app: AppKind;
  busy: boolean;
  canReset: boolean;
  extraConfigurationCount: number;
  onStart: () => void;
  onFinish: () => void;
  onSuccess: (message: CommandError) => void;
}

function useClientConfigurationResets({
  app,
  busy,
  canReset,
  extraConfigurationCount,
  onStart,
  onFinish,
  onSuccess,
}: UseClientConfigurationResetsProps) {
  const { t } = useI18n();
  const [pending, setPending] = useState<ResetState<PendingClientConfigurationReset | null>>(
    () => emptyResetState(null),
  );
  const [errors, setErrors] = useState<ResetState<unknown>>(() => emptyResetState(null));
  const [resetView, setResetView] = useState<ClientConfigurationResetKind>("nativeDefaults");
  const setPendingFor = (kind: ClientConfigurationResetKind, value: PendingClientConfigurationReset | null) => {
    setPending((current) => ({ ...current, [kind]: value }));
  };
  const setErrorFor = (kind: ClientConfigurationResetKind, value: unknown) => {
    setErrors((current) => ({ ...current, [kind]: value }));
  };
  const prepare = (resetKind: ClientConfigurationResetKind) => {
    if (!canReset || busy) return;
    setResetView(resetKind); setPendingFor(resetKind, null); setErrorFor(resetKind, null); onStart();
    void previewClientConfigurationReset(app, resetKind).then((preview) => {
      setPendingFor(resetKind, { preview, resetKind });
    }).catch((error: { message?: string }) => {
      setErrorFor(resetKind, error);
    }).finally(onFinish);
  };
  const commit = (resetKind: ClientConfigurationResetKind, successMessage: CommandError) => {
    const current = pending[resetKind];
    if (!current || busy || !canReset) return;
    setErrorFor(resetKind, null); onStart();
    void commitClientConfigurationReset(app, resetKind, current.preview).then(() => {
      setPending(emptyResetState(null));
      onSuccess(successMessage);
    }).catch((error: { message?: string }) => {
      setErrorFor(resetKind, error);
    }).finally(onFinish);
  };
  const inNativeView = resetView !== "clearExtraConfiguration";
  const commitNativeReset = () => {
    if (!inNativeView) return;
    commit(resetView, resetView === "nativeDefaults"
      ? uiMessage("clientConfig.status.nativeRestored")
      : uiMessage("clientConfig.status.nativeRestoredDeep"));
  };
  return {
    resetView,
    pendingNativeReset: inNativeView ? pending[resetView] : null,
    pendingExtraClear: pending.clearExtraConfiguration,
    nativeResetError: inNativeView && errors[resetView] != null ? errorText(errors[resetView], t) : null,
    extraClearError: errors.clearExtraConfiguration == null ? null : errorText(errors.clearExtraConfiguration, t),
    prepareNativeReset: (resetKind: NativeConfigurationResetKind) => prepare(resetKind),
    prepareExtraClear: () => prepare("clearExtraConfiguration"),
    commitNativeReset,
    commitExtraClear: () => commit("clearExtraConfiguration", uiMessage("clientConfig.status.extraCleared", { count: extraConfigurationCount })),
    showNativeReset: () => setResetView("nativeDefaults"),
  };
}

export function useClientConfigurationApply(
  props: UseClientConfigurationApplyProps,
  onCommitted: () => void,
) {
  const { editorState: state, app, busy } = props;
  const { t } = useI18n();
  const [pendingApply, setPendingApply] = useState<PendingClientConfiguration | null>(null);
  const [applyError, setApplyError] = useMessageState();
  const [statusMessage, setStatus] = useMessageState();
  const [applying, setApplying] = useState(false);
  const settings: SettingsValues | null = state.editor && state.draft
    ? clientSettingsPayload(app, state.draft, state.claudeExtra) : null;
  const subagentSettings = app === "codex" ? props.subagentDraft : undefined;
  const fileIsWritable = !props.configStatus?.exists || props.configStatus.syntaxOk;
  const recoveryBlocker = state.phase === "dirty" || props.hasUnsavedConfigurationDraft
    ? t("clientConfig.apply.unsavedBlocker")
    : null;
  const recoveryReady = fileIsWritable && state.phase === "clean" && !recoveryBlocker;
  const extraCount = app === "claude" ? extraConfigurationCount(state.editor?.settings.claudeExtra) : 0;
  const start = () => { setStatus(null); setApplying(true); };
  const finish = () => setApplying(false);
  const reset = useClientConfigurationResets({
    app,
    busy: busy || applying,
    canReset: recoveryReady,
    extraConfigurationCount: extraCount,
    onStart: start,
    onFinish: finish,
    onSuccess: (message) => { setStatus(message); onCommitted(); props.onApplied(); },
  });
  const prepareApply = () => {
    if (!settings || busy || applying) return;
    setPendingApply(null); setApplyError(null); start();
    void previewClientConfigurationApply(app, settings, subagentSettings).then((preview) => {
      setPendingApply({ preview, settings, subagentSettings });
    }).catch((error: { message?: string }) => {
      setApplyError(error);
    }).finally(finish);
  };
  const commitApply = () => {
    if (!pendingApply || busy || applying) return;
    setApplyError(null); start();
    void commitClientConfigurationApply(app, pendingApply.settings, pendingApply.preview, pendingApply.subagentSettings)
      .then(() => {
        setPendingApply(null);
        setStatus(uiMessage("clientConfig.status.applied"));
        onCommitted(); props.onApplied();
      })
      .catch((error: { message?: string }) => setApplyError(error))
      .finally(finish);
  };
  return {
    ...reset,
    applyError,
    applying,
    status: statusMessage === null ? null : { message: statusMessage, error: false },
    recoveryBlocker,
    extraConfigurationCount: extraCount,
    hasExtraConfiguration: extraCount > 0,
    canApply: fileIsWritable && !!settings && (app !== "codex" || !!subagentSettings),
    canNativeReset: recoveryReady,
    canClearExtra: recoveryReady && extraCount > 0,
    pendingApply,
    prepareApply,
    commitApply,
    cancelApply: () => setPendingApply(null),
  };
}

export type ClientConfigurationActionState = ReturnType<typeof useClientConfigurationApply>;

function ResetFailure({
  error,
  busy,
  onRetry,
  onBack,
}: {
  error: string;
  busy: boolean;
  onRetry: () => void;
  onBack?: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="asb-client-settings-reset-error" role="alert">
      <p className="asb-field-error">{error}</p>
      {onBack && <Button variant="secondary" disabled={busy} onClick={onBack}>{t("clientConfig.reset.backToSettings")}</Button>}
      <Button variant="secondary" disabled={busy} onClick={onRetry}>{t("clientConfig.reset.regeneratePreview")}</Button>
    </div>
  );
}

function ExtraConfigurationAction({
  configuration,
  busy,
}: {
  configuration: ClientConfigurationActionState;
  busy: boolean;
}) {
  const { t } = useI18n();
  if (!configuration.hasExtraConfiguration) return null;
  return (
    <section className="asb-client-extra-configuration" aria-label={t("clientConfig.extra.title")}>
      <div>
        <h3 className="asb-section-title">{t("clientConfig.extra.title")}</h3>
        <p className="asb-field-help">
          {t("clientConfig.extra.actionHelp", { count: configuration.extraConfigurationCount })}
        </p>
      </div>
      <Button
        variant="secondary"
        disabled={!configuration.canClearExtra || busy || configuration.applying}
        onClick={configuration.prepareExtraClear}
      >
        {t("clientConfig.reset.extraTitle")}
      </Button>
    </section>
  );
}

export function ClientConfigurationResetPanel({
  app,
  busy,
  configuration,
}: {
  app: AppKind;
  busy: boolean;
  configuration: ClientConfigurationActionState;
}) {
  const { t } = useI18n();
  const working = busy || configuration.applying;
  const nativeView: NativeConfigurationResetKind | null =
    configuration.resetView === "clearExtraConfiguration"
      ? null
      : configuration.resetView;
  if (nativeView === null) {
    if (configuration.pendingExtraClear) {
      return (
        <ClientConfigurationResetPreview
          preview={configuration.pendingExtraClear.preview}
          app={app}
          resetKind="clearExtraConfiguration"
          extraConfigurationCount={configuration.extraConfigurationCount}
          busy={working || !configuration.canClearExtra}
          onConfirm={configuration.commitExtraClear}
          onBack={configuration.showNativeReset}
        />
      );
    }
    if (configuration.extraClearError) {
      return <ResetFailure error={configuration.extraClearError} busy={working} onRetry={configuration.prepareExtraClear} onBack={configuration.showNativeReset} />;
    }
    return <p className="asb-field-help" role="status">{t("clientConfig.reset.generatingClear")}</p>;
  }
  if (configuration.pendingNativeReset) {
    return <>
      <ClientConfigurationResetPreview
        preview={configuration.pendingNativeReset.preview}
        app={app}
        resetKind={nativeView}
        extraConfigurationCount={configuration.extraConfigurationCount}
        busy={working || !configuration.canNativeReset}
        onConfirm={configuration.commitNativeReset}
        onScopeChange={configuration.prepareNativeReset}
      />
      <ExtraConfigurationAction configuration={configuration} busy={busy} />
    </>;
  }
  if (configuration.nativeResetError) {
    return (
      <ResetFailure
        error={configuration.nativeResetError}
        busy={working}
        onRetry={() => configuration.prepareNativeReset(nativeView)}
      />
    );
  }
  return <p className="asb-field-help" role="status">{t("clientConfig.reset.generatingNative")}</p>;
}

export function ClientConfigurationConfirmation({
  app,
  busy,
  applying,
  pending,
  onCancel,
  onConfirm,
}: {
  app: AppKind;
  busy: boolean;
  applying: boolean;
  pending: PendingClientConfiguration | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();
  if (!pending) return null;
  return (
    <ConfirmSheet
      title={t("clientConfig.apply.confirmTitle")}
      confirmLabel={t("clientConfig.apply.confirmButton")}
      confirmDisabled={busy || applying}
      busy={busy || applying}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <ul className="asb-dialog-details">
        <li>{t("clientConfig.apply.willWrite", { target: pending.preview.file.preview.target })}</li>
        <li><PreviewInspector filePreview={pending.preview.file} userConfigModel={null} userConfigWarnings={[]} /></li>
        <li>{app === "codex"
          ? t("clientConfig.apply.confirmCodexNote")
          : t("clientConfig.apply.confirmNote")}</li>
      </ul>
    </ConfirmSheet>
  );
}
