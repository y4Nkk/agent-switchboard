import { commandErrorText } from "../i18n/errors";
import { useEffect, useState, type ReactNode, type RefCallback } from "react";
import type { AppKind, CodexSubagentSettings, CodexSubagentSettingsSnapshot, SettingValue, ConfigFileStatus, ConfigurationSources } from "../api/client";
import type { ClientSettingsEditorState } from "../app/useClientSettings";
import { useI18n, type TFunction } from "../i18n";
import { AppDialog, AppDialogSuspension } from "./AppDialog";
import { Button } from "./Button";
import { ClientPicker } from "./ClientPicker";
import { CurrentConfigurationEditor, type ConfigurationEditState } from "./CurrentConfigurationEditor";
import {
  ClientConfigurationConfirmation,
  ClientConfigurationResetPanel,
  type ClientConfigurationActionState,
  useClientConfigurationApply,
} from "./ClientConfigurationApply";
import { ClaudeExtraConfigurationEditor } from "./ClaudeExtraConfigurationEditor";
import { OfficialSettingsDirectory } from "./OfficialSettingsDirectory";
import { ScrollArea } from "./ScrollArea";
import { SettingsFields } from "./SettingsFields";
import { SettingsLayout } from "./SettingsLayout";
import { CodexPermissionProfile } from "./CodexPermissionProfile";
import { ClientConfigurationSourcesDialog, useClientConfigurationSources } from "./ClientConfigurationSources";
import { WorkspaceHeader } from "./WorkspaceHeader";
import { useUnsavedChanges } from "./useUnsavedChanges";

interface ClientSettingsPanelProps {
  /** Whether this client's page is on screen. The header entries open
   * dialogs, and a dialog never outlives the page or client route owning it. */
  active: boolean;
  routeContentRef: RefCallback<HTMLElement>;
  app: AppKind;
  onSelectApp: (app: AppKind) => void;
  editorState: ClientSettingsEditorState;
  busy: boolean;
  configStatus: ConfigFileStatus | undefined;
  hasUnsavedConfigurationDraft: boolean;
  onValueChange: (app: AppKind, key: string, value: SettingValue) => void;
  onKeepFileValue: (app: AppKind, key: string) => void;
  onPermissionProfileChange: (value: SettingValue) => void;
  onApplied: () => void;
  onFileApplied: () => void;
  onDiscardSettings: () => void;
  onRetryLoad: (app: AppKind) => void;
  onReview: (app: AppKind) => void;
  onClaudeExtraChange: (app: AppKind, extra: Record<string, unknown>) => void;
  globalInstructions: ReactNode;
  /** The draft facts behind `globalInstructions`, read by that dialog's close guard. */
  globalInstructionsDirty: boolean;
  onDiscardGlobalInstructions: () => void;
  /** Codex's directly-applied subagent resource sits after model behavior,
   * outside the application-owned client-preference store. */
  renderSubagentSettings?: (busy: boolean) => ReactNode;
  subagentDraft?: CodexSubagentSettings;
  subagentBaseline?: CodexSubagentSettingsSnapshot;
}

function actionStatus(t: TFunction, props: ClientSettingsPanelProps) {
  if (props.configStatus?.readError) {
    return t("providers.route.notes.readError", { detail: props.configStatus.readError });
  }
  if (props.configStatus?.exists && !props.configStatus.syntaxOk) {
    return t("clientConfig.status.syntaxError");
  }
  const sourceError = props.configStatus?.clientSettingsError ?? props.editorState.editor?.sourceError;
  if (sourceError) {
    return t("clientConfig.status.extractError", { message: sourceError });
  }
  return null;
}

interface ConfigurationReviewProps extends ClientSettingsPanelProps {
  onManualEditStateChange: (state: ConfigurationEditState) => void;
  onExtraEditStateChange: (state: ConfigurationEditState) => void;
}

function ClientConfigurationReview({
  editorState: state,
  busy,
  app,
  onFileApplied,
  onClaudeExtraChange,
  onManualEditStateChange,
  onExtraEditStateChange,
}: ConfigurationReviewProps) {
  const { t } = useI18n();
  const source = state.currentConfiguration;
  if (!source && !state.currentConfigurationLoading && !state.currentConfigurationError) {
    return <p className="asb-field-help" role="status">{t("clientConfig.review.preparing")}</p>;
  }
  return (
    <>
      {state.currentConfigurationLoading && <p className="asb-field-help" role="status">{t("clientConfig.review.readingReal")}</p>}
      {state.currentConfigurationError && (
        <p className="asb-field-error" role="alert">
          {t("clientConfig.review.readError", { message: commandErrorText(state.currentConfigurationError, t) })}
        </p>
      )}
      {source && (
        <CurrentConfigurationEditor
          app={app}
          busy={busy}
          source={source}
          onApplied={onFileApplied}
          onEditStateChange={onManualEditStateChange}
        />
      )}
      <section className="asb-client-configuration-review-scope" aria-label={t("clientConfig.review.scopeTitle")}>
        <h3 className="asb-section-title">{t("clientConfig.review.scopeTitle")}</h3>
        <p className="asb-field-help">
          {t("clientConfig.review.scopeHelp")}
          {app === "codex" && <>{" "}{t("clientConfig.review.scopeCodexHelp")}</>}
        </p>
      </section>
      {app === "claude" && (
        <ClaudeExtraConfigurationEditor
          extra={state.claudeExtra}
          busy={busy}
          onChange={(extra) => onClaudeExtraChange(app, extra)}
          onEditStateChange={onExtraEditStateChange}
        />
      )}
    </>
  );
}
function ClientPreferencesEditor(props: ClientSettingsPanelProps & {
  report: ConfigurationSources | null;
  onInspectSources: () => void;
}) {
  const { editorState: state, app, busy } = props;
  const { t } = useI18n();
  const subagentSettings = props.renderSubagentSettings?.(busy);
  if (state.phase === "idle" || state.phase === "loading")
    return <>
      {subagentSettings}
      <div className="asb-settings-skeleton" role="status" aria-label={t("clientConfig.loading")}>
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
      </div>
    </>;
  if (state.phase === "loadError" || !state.editor || !state.draft) {
    return (
      <>
        {subagentSettings}
        <div className="asb-empty" role="alert">
          <p>
            {t("clientConfig.loadError", { message: state.error ? commandErrorText(state.error, t) : t("clientConfig.loadErrorFallback") })}
          </p>
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => props.onRetryLoad(app)}
          >
            {t("clientConfig.common.reload")}
          </Button>
        </div>
      </>
    );
  }
  const source = state.editor.source;
  const fieldProps = {
    specs: state.editor.specs,
    values: state.draft,
    baselineValues: source?.values,
    savedValues: state.editor.settings.settings,
    keepFileValues: state.keepFileValues,
    actualValues: props.configStatus ? props.configStatus.clientSettings?.settings : source?.values,
    supportedSettings: props.report?.supportedSettings,
    busy: busy || !source,
    onChange: (key: string, value: SettingValue) => props.onValueChange(app, key, value),
    onKeepFileValue: (key: string) => props.onKeepFileValue(app, key),
    clientApp: app,
  };
  // The backend catalog emits stable group keys; the display name resolves
  // through the catalog at render time.
  const modelBehaviorGroups = state.editor.groups.filter((group) => group === "ownership.group.modelBehavior");
  const remainingGroups = state.editor.groups.filter((group) => group !== "ownership.group.modelBehavior");
  return (
    <>
      {modelBehaviorGroups.length > 0 && (
        <SettingsFields
          {...fieldProps}
          groups={modelBehaviorGroups}
        />
      )}
      {subagentSettings}
      {remainingGroups.length > 0 && (
        <>
          {app === "codex" && <CodexPermissionProfile status={props.configStatus} report={props.report}
            baseline={source?.permissionProfile} draft={state.permissionProfile} busy={busy}
            inspect={props.onInspectSources} onChange={props.onPermissionProfileChange} />}
          <SettingsFields {...fieldProps} groups={remainingGroups} />
        </>
      )}
    </>
  );
}

type ClientSettingsSection = "directory" | "review" | "instructions" | "reset" | "sources";

/** The client picker and configuration tools share the workspace header. */
function ClientSettingsToolbar({
  panel,
  configuration,
  onOpen,
}: {
  panel: ClientSettingsPanelProps;
  configuration: ClientConfigurationActionState;
  onOpen: (section: ClientSettingsSection) => void;
}) {
  const { t } = useI18n();
  const working = panel.busy || configuration.applying;
  return (
    <WorkspaceHeader
      title={t("clientConfig.title")}
      primary={
        <ClientPicker
          app={panel.app}
          disabled={working}
          label={t("clientConfig.pickerLabel")}
          onChange={(target) => panel.onSelectApp(target)}
        />
      }
      primaryActions={
        <>
          <Button
            variant="secondary"
            disabled={working || !panel.editorState.editor}
            onClick={() => onOpen("directory")}
          >
            {t("clientConfig.toolbar.directory")}
          </Button>
          <Button variant="secondary" disabled={working} onClick={() => onOpen("sources")}>
            {t("clientConfig.sources.title")}
          </Button>
          <Button
            variant="secondary"
            disabled={working}
            onClick={() => {
              onOpen("review");
              panel.onReview(panel.app);
            }}
          >
            {t("clientConfig.review.title")}
          </Button>
          <Button
            variant="secondary"
            disabled={working}
            onClick={() => onOpen("instructions")}
          >
            {t("clientConfig.toolbar.instructions")}
          </Button>
          <Button
            variant="danger"
            disabled={!configuration.canNativeReset || panel.busy || configuration.applying}
            onClick={() => {
              onOpen("reset");
              configuration.prepareNativeReset();
            }}
          >
            {t("clientConfig.reset.nativeTitle")}
          </Button>
        </>
      }
    />
  );
}

function ClientSettingsActions({ panel, configuration }: {
  panel: ClientSettingsPanelProps;
  configuration: ClientConfigurationActionState;
}) {
  const { t } = useI18n();
  const issue = actionStatus(t, panel);
  const { pendingCount } = configuration;
  const summary = pendingCount
    ? t("clientConfig.status.pendingCount", { count: pendingCount })
    : panel.hasUnsavedConfigurationDraft ? t("clientConfig.status.dirty")
      : configuration.status?.message;
  const previewError = configuration.pendingApply ? null : configuration.applyError;
  return (
    <>
      {issue && <p className="asb-client-configuration-action-error asb-field-error" role="alert">{issue}</p>}
      {previewError && <p className="asb-client-configuration-action-error asb-field-error" role="alert">{previewError}</p>}
      <div className="asb-scroll-action-buttons">
        {!issue && summary && (
          <p className="asb-client-configuration-summary" data-pending={pendingCount > 0 || undefined} role="status">{summary}</p>
        )}
        {panel.hasUnsavedConfigurationDraft && <Button variant="secondary" disabled={panel.busy || configuration.applying}
          onClick={panel.onDiscardSettings}>{t("clientConfig.settings.discardDraft")}</Button>}
        <Button variant="primary" disabled={!configuration.canApply || panel.busy || configuration.applying}
          onClick={configuration.prepareApply}>
          {t("clientConfig.apply.saveAndPreview")}
        </Button>
      </div>
    </>
  );
}

interface ClientSettingsDialogProps {
  panel: ClientSettingsPanelProps;
  onClose: () => void;
}

/** A manual edit that is not applied yet asks before the dialog closes; the
 * confirmation is what drops it, and a running write locks the dialog. */
function ConfigurationReviewDialog({ panel, onClose }: ClientSettingsDialogProps) {
  const { t } = useI18n();
  const [manual, setManual] = useState({ dirty: false, working: false });
  const [extra, setExtra] = useState({ dirty: false, working: false });
  const working = panel.busy || manual.working || extra.working;
  const leave = useUnsavedChanges(manual.dirty || extra.dirty, working);
  return <>
    <AppDialogSuspension suspended={Boolean(leave.confirmation)}>
      <AppDialog title={t("clientConfig.review.title")} busy={working} wide onClose={() => leave.request(onClose)}>
        <ClientConfigurationReview {...panel} onManualEditStateChange={setManual} onExtraEditStateChange={setExtra} />
      </AppDialog>
    </AppDialogSuspension>
    {leave.confirmation}
  </>;
}

/** The instruction draft outlives the dialog in the prompt model, so closing
 * asks first and the confirmation is what discards that draft. */
function GlobalInstructionsDialog({ panel, onClose }: ClientSettingsDialogProps) {
  const { t } = useI18n();
  const leave = useUnsavedChanges(panel.globalInstructionsDirty, panel.busy);
  const discardAndClose = () => {
    panel.onDiscardGlobalInstructions();
    onClose();
  };
  return <>
    <AppDialogSuspension suspended={Boolean(leave.confirmation)}>
      <AppDialog title={t("clientConfig.toolbar.instructions")} busy={panel.busy}
        onClose={() => leave.request(discardAndClose)}>
        {panel.globalInstructions}
      </AppDialog>
    </AppDialogSuspension>
    {leave.confirmation}
  </>;
}

/** The title names the view being confirmed: the native restore, or the
 * extra-settings clear reached from the action under its preview. */
function NativeResetDialog({
  panel,
  onClose,
  configuration,
}: ClientSettingsDialogProps & { configuration: ClientConfigurationActionState }) {
  const { t } = useI18n();
  return (
    <AppDialog
      title={configuration.resetView === "clearExtraConfiguration"
        ? t("clientConfig.reset.extraTitle")
        : t("clientConfig.reset.nativeTitle")}
      busy={panel.busy || configuration.applying}
      onClose={onClose}
      wide
    >
      <ClientConfigurationResetPanel app={panel.app} busy={panel.busy} configuration={configuration} />
    </AppDialog>
  );
}

export function ClientSettingsPanel(props: ClientSettingsPanelProps) {
  const { t } = useI18n();
  const [openSection, setOpenSection] = useState<ClientSettingsSection | null>(null);
  const sources = useClientConfigurationSources(props.app);
  // A modal left inside a hidden page or client route would keep the rest of
  // the application inert, so it closes with the surface that owns it.
  useEffect(() => {
    if (!props.active) setOpenSection(null);
  }, [props.active]);
  const configuration = useClientConfigurationApply({ ...props, directory: sources.directory }, () => setOpenSection(null));
  const open = (next: ClientSettingsSection) => {
    setOpenSection(next);
    if (next === "sources") sources.check();
  };
  const onClose = () => setOpenSection(null);
  const dialog = { panel: props, onClose };
  const section = props.active ? openSection : null;
  return <>
    <div className="asb-client-settings-panel" aria-label={t("clientConfig.title")}>
      <ScrollArea tabIndex={0} aria-label={t("clientConfig.form.settingsAria")} floatingActions={{
        label: t("clientConfig.form.actionsAria"),
        content: <ClientSettingsActions panel={props} configuration={configuration} />,
      }}>
        <ClientSettingsToolbar panel={props} configuration={configuration} onOpen={open} />
        <SettingsLayout ref={props.routeContentRef}>
          <ClientPreferencesEditor {...props} busy={props.busy || configuration.applying}
            report={sources.report} onInspectSources={() => open("sources")} />
        </SettingsLayout>
      </ScrollArea>
    </div>
    {section === "directory" && (
      <AppDialog title={t("clientConfig.toolbar.directory")} busy={props.busy} onClose={onClose} wide tall>
        <OfficialSettingsDirectory entries={props.editorState.editor?.directory ?? []} />
      </AppDialog>
    )}
    {section === "review" && <ConfigurationReviewDialog {...dialog} />}
    {section === "sources" && <ClientConfigurationSourcesDialog app={props.app} sources={sources} onClose={onClose} />}
    {section === "instructions" && <GlobalInstructionsDialog {...dialog} />}
    {section === "reset" && <NativeResetDialog {...dialog} configuration={configuration} />}
    {props.active && <ClientConfigurationConfirmation
      app={props.app}
      busy={props.busy}
      applying={configuration.applying}
      pending={configuration.pendingApply}
      error={configuration.applyError}
      onConfirm={configuration.commitApply}
      onCancel={configuration.cancelApply}
    />}
  </>;
}
