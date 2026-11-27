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
  type ClientConfigurationChanges,
  type ClientConfigurationChangesPreview,
  type ClientConfigurationResetPreview as ResetPreview,
  type ClientConfigurationResetKind,
  type CodexSubagentSettings,
  type CodexSubagentSettingsSnapshot,
  type ConfigFileStatus,
} from "../api/client";
import { sameClaudeExtra } from "../app/claude-common-settings";
import { changedSettingValues, type ClientSettingsEditorState } from "../app/useClientSettings";
import { sameSettingValue } from "../lib/setting-values";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { PreviewInspector } from "./PreviewInspector";
import { ClientConfigurationResetPreview } from "./ClientConfigurationResetPreview";

export interface PendingClientConfiguration {
  preview: ClientConfigurationChangesPreview;
  changes: ClientConfigurationChanges;
  directory: string | null;
}

export interface PendingClientConfigurationReset {
  preview: ResetPreview;
  selected: string[];
}

interface UseClientConfigurationApplyProps {
  app: AppKind;
  busy: boolean;
  editorState: ClientSettingsEditorState;
  subagentDraft?: CodexSubagentSettings;
  subagentBaseline?: CodexSubagentSettingsSnapshot;
  directory: string | null;
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
  const prepare = (resetKind: ClientConfigurationResetKind, selected: string[] = [], sourceHash: string | null = null) => {
    if (!canReset || busy) return;
    setResetView(resetKind);
    if (sourceHash === null) setPendingFor(resetKind, null);
    setErrorFor(resetKind, null); onStart();
    void previewClientConfigurationReset(app, resetKind, selected, sourceHash).then((preview) => {
      setPendingFor(resetKind, { preview, selected });
    }).catch((error: { message?: string }) => {
      setErrorFor(resetKind, error);
    }).finally(onFinish);
  };
  const commit = (resetKind: ClientConfigurationResetKind, successMessage: CommandError) => {
    const current = pending[resetKind];
    if (!current || busy || !canReset || errors[resetKind] != null) return;
    setErrorFor(resetKind, null); onStart();
    void commitClientConfigurationReset(app, resetKind, current.selected, current.preview).then(() => {
      setPending(emptyResetState(null));
      onSuccess(successMessage);
    }).catch((error: { message?: string }) => {
      setErrorFor(resetKind, error);
    }).finally(onFinish);
  };
  const native = pending.nativeDefaults;
  const commitNativeReset = () => {
    commit("nativeDefaults", native?.selected.length ? uiMessage("clientConfig.status.nativeRestoredDeep")
      : uiMessage("clientConfig.status.nativeRestored"));
  };
  return {
    resetView,
    pendingNativeReset: native,
    pendingExtraClear: pending.clearExtraConfiguration,
    nativeResetError: errors.nativeDefaults == null ? null : errorText(errors.nativeDefaults, t),
    extraClearError: errors.clearExtraConfiguration == null ? null : errorText(errors.clearExtraConfiguration, t),
    prepareNativeReset: () => prepare("nativeDefaults"),
    selectResetField: (key: string, checked: boolean) => {
      if (!native) return;
      const selected = checked ? [...native.selected, key] : native.selected.filter((entry) => entry !== key);
      prepare("nativeDefaults", selected, native.preview.file.contentHash);
    },
    prepareExtraClear: () => prepare("clearExtraConfiguration"),
    commitNativeReset,
    commitExtraClear: () => commit("clearExtraConfiguration", uiMessage("clientConfig.status.extraCleared", { count: extraConfigurationCount })),
    showNativeReset: () => setResetView("nativeDefaults"),
  };
}

function prepareChanges(props: UseClientConfigurationApplyProps): ClientConfigurationChanges | null {
  const { editorState: state, subagentBaseline, subagentDraft } = props;
  const source = state.editor?.source;
  if (!source || !state.draft) return null;
  const settings = changedSettingValues(state.draft, source.values);
  const changes: ClientConfigurationChanges = { settings };
  if (state.keepFileValues?.length) changes.keepFileValues = state.keepFileValues;
  if (state.permissionProfile) changes.permissionProfile = state.permissionProfile;
  if (!sameClaudeExtra(state.claudeExtra, state.editor?.settings.claudeExtra)) {
    changes.claudeExtra = state.claudeExtra ?? {};
  }
  if (subagentBaseline && subagentDraft) {
    const before = subagentBaseline.settings;
    const fields = (Object.keys(before) as (keyof CodexSubagentSettings)[])
      .filter((key) => !sameSettingValue(before[key], subagentDraft[key]));
    if (fields.length) changes.subagents = Object.fromEntries(fields.map((key) => [key, subagentDraft[key]]));
  }
  return Object.keys(settings).length || changes.keepFileValues || changes.claudeExtra || changes.subagents || changes.permissionProfile ? changes : null;
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
  const changes = prepareChanges(props);
  const pendingCount = changes
    ? new Set([...Object.keys(changes.settings), ...(changes.keepFileValues ?? [])]).size
      + Object.keys(changes.subagents ?? {}).length
      + Number(changes.permissionProfile !== undefined)
      + Number(changes.claudeExtra !== undefined)
    : 0;
  const source = state.editor?.source;
  const subagentStale = !!changes?.subagents && (props.subagentBaseline?.configHash !== source?.contentHash
    || props.subagentBaseline?.fileExists !== source?.exists);
  const fileIsWritable = !props.configStatus?.readError && (!props.configStatus?.exists || props.configStatus.syntaxOk);
  const recoveryReady = fileIsWritable && state.phase === "clean" && !props.hasUnsavedConfigurationDraft;
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
    if (!changes || !source || !state.editor || subagentStale || !fileIsWritable || busy || applying) return;
    setPendingApply(null); setApplyError(null); start();
    const directory = changes.permissionProfile ? props.directory : null;
    void previewClientConfigurationApply(app, source, state.editor.settingsHash, changes, directory).then((preview) => {
      setPendingApply({ preview, changes, directory });
    }).catch((error: { message?: string }) => {
      setApplyError(error);
    }).finally(finish);
  };
  const commitApply = () => {
    if (!pendingApply || busy || applying) return;
    setApplyError(null); start();
    void commitClientConfigurationApply(app, pendingApply.changes, pendingApply.preview, pendingApply.directory)
      .then(() => {
        setPendingApply(null);
        setStatus(uiMessage(pendingApply.preview.file.contentHash === pendingApply.preview.file.renderedHash
          ? "clientConfig.status.savedIntent" : "clientConfig.status.applied"));
        onCommitted(); props.onApplied();
      })
      .catch((error: { message?: string }) => setApplyError(error))
      .finally(finish);
  };
  return {
    ...reset,
    applyError: subagentStale ? t("clientConfig.status.staleDraft") : applyError,
    applying,
    status: statusMessage === null ? null : { message: statusMessage, error: false },
    pendingCount,
    extraConfigurationCount: extraCount,
    hasExtraConfiguration: extraCount > 0,
    canApply: fileIsWritable && !!changes && !subagentStale,
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
  if (configuration.resetView === "clearExtraConfiguration") {
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
  if (configuration.nativeResetError) {
    return <ResetFailure error={configuration.nativeResetError} busy={working} onRetry={configuration.prepareNativeReset} />;
  }
  if (configuration.pendingNativeReset) {
    return <>
      <ClientConfigurationResetPreview
        preview={configuration.pendingNativeReset.preview}
        app={app}
        resetKind="nativeDefaults"
        extraConfigurationCount={configuration.extraConfigurationCount}
        busy={working || !configuration.canNativeReset}
        onConfirm={configuration.commitNativeReset}
        selected={configuration.pendingNativeReset.selected}
        onSelect={configuration.selectResetField}
      />
      <ExtraConfigurationAction configuration={configuration} busy={busy} />
    </>;
  }
  return <p className="asb-field-help" role="status">{t("clientConfig.reset.generatingNative")}</p>;
}

export function ClientConfigurationConfirmation({
  app,
  busy,
  applying,
  pending,
  error,
  onCancel,
  onConfirm,
}: {
  app: AppKind;
  busy: boolean;
  applying: boolean;
  pending: PendingClientConfiguration | null;
  error: string | null;
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
      {error && <p className="asb-field-error" role="alert">{error}</p>}
      <ul className="asb-dialog-details">
        {pending.preview.file.contentHash !== pending.preview.file.renderedHash && <>
          <li>{t("clientConfig.apply.willWrite", { target: pending.preview.file.preview.target })}</li>
          <li><PreviewInspector filePreview={pending.preview.file} userConfigModel={null} userConfigWarnings={[]} /></li>
        </>}
        {pending.changes.keepFileValues?.length ? <li>
          {t("clientConfig.apply.keepFileValues")}
          <ul>{pending.changes.keepFileValues.map((key) => <li key={key}><code>{key}</code></li>)}</ul>
        </li> : null}
        <li>{app === "codex"
          ? t("clientConfig.apply.confirmCodexNote")
          : t("clientConfig.apply.confirmNote")}</li>
      </ul>
    </ConfirmSheet>
  );
}
