import { commandErrorText } from "../i18n/errors";
import { useId, useRef, useState, type ReactNode, type RefCallback } from "react";
import { type AppKind, type CodexSubagentSettings, type SettingValue, type ConfigFileStatus } from "../api/client";
import type { ClientSettingsEditorState } from "../app/useClientSettings";
import { useI18n, type TFunction } from "../i18n";
import { Button } from "./Button";
import { ClientPicker } from "./ClientPicker";
import { Collapsible } from "./Collapsible";
import { CurrentConfigurationEditor } from "./CurrentConfigurationEditor";
import {
  ClientConfigurationConfirmation,
  ClientConfigurationResetPanel,
  type ClientConfigurationActionState,
  useClientConfigurationApply,
} from "./ClientConfigurationApply";
import { ClaudeExtraConfigurationEditor } from "./ClaudeExtraConfigurationEditor";
import { OfficialSettingsDirectory } from "./OfficialSettingsDirectory";
import { SettingsFields } from "./SettingsFields";
import { CodexPermissionProfileNotice } from "./CodexPermissionProfileNotice";
import { WorkspaceHeader } from "./WorkspaceHeader";

interface ClientSettingsPanelProps {
  routeContentRef: RefCallback<HTMLElement>;
  app: AppKind;
  onSelectApp: (app: AppKind) => void;
  editorState: ClientSettingsEditorState;
  busy: boolean;
  configStatus: ConfigFileStatus | undefined;
  hasUnsavedConfigurationDraft: boolean;
  onValueChange: (app: AppKind, key: string, value: SettingValue) => void;
  onApplied: () => void;
  onRetryLoad: (app: AppKind) => void;
  onReview: (app: AppKind) => void;
  onClaudeExtraChange: (app: AppKind, extra: Record<string, unknown>) => void;
  globalInstructions: ReactNode;
  /** Codex's directly-applied subagent resource sits after model behavior,
   * outside the application-owned client-preference store. */
  subagentSettings?: ReactNode;
  subagentDraft?: CodexSubagentSettings;
}

function actionStatus(t: TFunction, props: ClientSettingsPanelProps) {
  if (props.configStatus?.readError) {
    return { message: t("providers.route.notes.readError", { detail: props.configStatus.readError }), error: true };
  }
  if (props.configStatus?.exists && !props.configStatus.syntaxOk) {
    return { message: t("clientConfig.status.syntaxError"), error: true };
  }
  if (props.editorState.phase === "dirty") return { message: t("clientConfig.status.dirty"), error: false };
  return null;
}

interface ConfigurationReviewProps extends ClientSettingsPanelProps {
  open: boolean;
}

interface ConfigurationReviewButtonProps extends ConfigurationReviewProps {
  reviewId: string;
  onToggle: (trigger: HTMLButtonElement) => void;
}

function ClientConfigurationReviewButton({
  editorState: state,
  busy,
  open,
  reviewId,
  onToggle,
}: ConfigurationReviewButtonProps) {
  const { t } = useI18n();
  const label = state.currentConfigurationLoading
    ? t("clientConfig.review.reading")
    : open ? t("clientConfig.review.collapse") : t("clientConfig.review.title");
  return (
    <Button
      variant="secondary"
      aria-expanded={open}
      aria-controls={open ? reviewId : undefined}
      disabled={busy || state.currentConfigurationLoading}
      onClick={(event) => onToggle(event.currentTarget)}
    >
      {label}
    </Button>
  );
}

function ClientConfigurationReview({
  editorState: state,
  busy,
  app,
  onApplied,
  subagentDraft,
  onClaudeExtraChange,
}: ConfigurationReviewProps) {
  const { t } = useI18n();
  const source = state.currentConfiguration;
  if (!source && !state.currentConfigurationLoading && !state.currentConfigurationError) {
    return <p className="asb-field-help" role="status">{t("clientConfig.review.preparing")}</p>;
  }
  return (
    <div className="asb-client-configuration-review">
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
          editorState={state}
          source={source}
          subagentDraft={subagentDraft}
          onApplied={onApplied}
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
        />
      )}
    </div>
  );
}
function ClientPreferencesEditor(props: ClientSettingsPanelProps) {
  const { editorState: state, app, busy } = props;
  const { t } = useI18n();
  if (state.phase === "idle" || state.phase === "loading")
    return <>
      {props.subagentSettings}
      <div className="asb-settings-skeleton" role="status" aria-label={t("clientConfig.loading")}>
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
      </div>
    </>;
  if (state.phase === "loadError" || !state.editor || !state.draft) {
    return (
      <>
        {props.subagentSettings}
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
  const working = busy;
  // The backend catalog emits stable group keys; the display name resolves
  // through the catalog at render time.
  const modelBehaviorGroups = state.editor.groups.filter((group) => group === "ownership.group.modelBehavior");
  const remainingGroups = state.editor.groups.filter((group) => group !== "ownership.group.modelBehavior");
  return (
    <>
      {modelBehaviorGroups.length > 0 && (
        <SettingsFields
          specs={state.editor.specs}
          groups={modelBehaviorGroups}
          values={state.draft}
          baselineValues={state.editor.settings.settings}
          actualValues={props.configStatus ? (props.configStatus.clientSettings?.settings ?? {}) : undefined}
          busy={working}
          onChange={(key, value) => props.onValueChange(app, key, value)}
          showGroupReset={false}
          presentation="client"
          clientApp={app}
        />
      )}
      {props.subagentSettings}
      {remainingGroups.length > 0 && (
        <>
        {app === "codex" && <CodexPermissionProfileNotice status={props.configStatus} />}
        <SettingsFields
          specs={state.editor.specs}
          groups={remainingGroups}
          values={state.draft}
          baselineValues={state.editor.settings.settings}
          actualValues={props.configStatus ? (props.configStatus.clientSettings?.settings ?? {}) : undefined}
          busy={working}
          onChange={(key, value) => props.onValueChange(app, key, value)}
          showGroupReset={false}
          presentation="client"
          clientApp={app}
        />
        </>
      )}
    </>
  );
}

type ClientSettingsSection = "directory" | "review" | "instructions" | "reset";

interface ClientSettingsToolbarProps {
  panel: ClientSettingsPanelProps;
  openSection: ClientSettingsSection | null;
  directoryId: string;
  reviewId: string;
  instructionsId: string;
  resetId: string;
  configuration: ClientConfigurationActionState;
  onSectionChange: (section: ClientSettingsSection | null, trigger: HTMLButtonElement) => void;
}

/** The page header: the client picker leads the navigation row, the four
 * disclosure entries sit right of it like every workspace's page actions. */
function ClientSettingsToolbar({
  panel,
  openSection,
  directoryId,
  reviewId,
  instructionsId,
  resetId,
  configuration,
  onSectionChange,
}: ClientSettingsToolbarProps) {
  const { t } = useI18n();
  const directoryOpen = openSection === "directory";
  const reviewOpen = openSection === "review";
  const instructionsOpen = openSection === "instructions";
  const resetOpen = openSection === "reset";
  return (
    <WorkspaceHeader
      title={t("clientConfig.title")}
      primary={
        <ClientPicker
          app={panel.app}
          disabled={panel.busy}
          label={t("clientConfig.pickerLabel")}
          onChange={(target) => panel.onSelectApp(target)}
        />
      }
      primaryActions={
        <>
          <Button
            variant="secondary"
            aria-expanded={directoryOpen}
            aria-controls={directoryOpen ? directoryId : undefined}
            disabled={!panel.editorState.editor}
            onClick={(event) => onSectionChange(directoryOpen ? null : "directory", event.currentTarget)}
          >
            {directoryOpen ? t("clientConfig.toolbar.backFromDirectory") : t("clientConfig.toolbar.directory")}
          </Button>
          <ClientConfigurationReviewButton
            {...panel}
            open={reviewOpen}
            reviewId={reviewId}
            onToggle={(trigger) => {
              const next = !reviewOpen;
              onSectionChange(next ? "review" : null, trigger);
              if (next) panel.onReview(panel.app);
            }}
          />
          <Button
            variant="secondary"
            aria-expanded={instructionsOpen}
            aria-controls={instructionsOpen ? instructionsId : undefined}
            disabled={panel.busy}
            onClick={(event) => onSectionChange(instructionsOpen ? null : "instructions", event.currentTarget)}
          >
            {instructionsOpen ? t("clientConfig.toolbar.collapseInstructions") : t("clientConfig.toolbar.instructions")}
          </Button>
          <Button
            variant="secondary"
            aria-expanded={resetOpen}
            aria-controls={resetOpen ? resetId : undefined}
            disabled={!configuration.canNativeReset || panel.busy || configuration.applying}
            onClick={(event) => {
              const next = !resetOpen;
              onSectionChange(next ? "reset" : null, event.currentTarget);
              if (next) configuration.prepareNativeReset("nativeDefaults");
            }}
          >
            {resetOpen && configuration.applying
              ? configuration.resetView === "clearExtraConfiguration" ? t("clientConfig.toolbar.generatingClear") : t("clientConfig.toolbar.generatingNative")
              : resetOpen ? t("clientConfig.toolbar.collapseReset") : t("clientConfig.reset.nativeTitle")}
          </Button>
        </>
      }
    />
  );
}

function ClientSettingsForm({
  panel,
  directoryOpen,
  reviewOpen,
  resetOpen,
  configuration,
}: {
  panel: ClientSettingsPanelProps;
  directoryOpen: boolean;
  reviewOpen: boolean;
  resetOpen: boolean;
  configuration: ClientConfigurationActionState;
}) {
  const { t } = useI18n();
  const status = configuration.status ?? actionStatus(t, panel);
  return <div hidden={directoryOpen || reviewOpen || resetOpen}>
    {panel.configStatus?.clientSettingsError && (
      <p className="asb-field-error" role="alert">
        {t("clientConfig.status.extractError", { message: panel.configStatus.clientSettingsError })}
      </p>
    )}
    <ClientPreferencesEditor {...panel} />
    <section className="asb-client-configuration-actions" aria-label={t("clientConfig.form.actionsAria")}>
      <div className="asb-client-configuration-notice">
        {status && <span className={status.error ? "asb-field-error" : "asb-field-help"} role={status.error ? "alert" : "status"}>{status.message}</span>}
        {configuration.recoveryBlocker && <p className="asb-field-help">{configuration.recoveryBlocker}</p>}
        {configuration.applyError && <p className="asb-field-error" role="alert">{configuration.applyError}</p>}
      </div>
      <div className="asb-client-configuration-commit">
        <Button variant="primary" disabled={!configuration.canApply || panel.busy || configuration.applying}
          onClick={configuration.prepareApply}>
          {t("clientConfig.apply.saveAndPreview")}
        </Button>
      </div>
    </section>
  </div>;
}

export function ClientSettingsPanel(props: ClientSettingsPanelProps) {
  const { t } = useI18n();
  const [openSection, setOpenSection] = useState<ClientSettingsSection | null>(null);
  const directoryId = useId();
  const reviewId = useId();
  const instructionsId = useId();
  const resetId = useId();
  const panelTrigger = useRef<HTMLButtonElement>(null);
  const directoryOpen = openSection === "directory";
  const reviewOpen = openSection === "review";
  const instructionsOpen = openSection === "instructions";
  const resetOpen = openSection === "reset";
  const changeSection = (section: ClientSettingsSection | null, trigger: HTMLButtonElement) => {
    panelTrigger.current = trigger;
    setOpenSection(section);
  };
  const configuration = useClientConfigurationApply(props, () => setOpenSection(null));
  return <>
    <div className="asb-client-settings-panel" aria-label={t("clientConfig.title")} onKeyDown={(event) => {
      if (event.key !== "Escape" || !openSection) return;
      event.preventDefault(); setOpenSection(null); panelTrigger.current?.focus();
    }}>
      <ClientSettingsToolbar
        panel={props}
        openSection={openSection}
        directoryId={directoryId}
        reviewId={reviewId}
        instructionsId={instructionsId}
        resetId={resetId}
        configuration={configuration}
        onSectionChange={changeSection}
      />
      <div ref={props.routeContentRef}>
        <Collapsible open={reviewOpen} id={reviewId} ariaLabel={t("clientConfig.review.title")}>
          <ClientConfigurationReview {...props} open={reviewOpen} />
        </Collapsible>
        <Collapsible open={instructionsOpen} id={instructionsId} className="is-instructions"
          ariaLabel={t("clientConfig.toolbar.instructionsEditor")}>
          <div className="asb-client-settings-instructions">{props.globalInstructions}</div>
        </Collapsible>
        <Collapsible open={directoryOpen} id={directoryId} ariaLabel={t("clientConfig.toolbar.directory")}>
          <OfficialSettingsDirectory entries={props.editorState.editor?.directory ?? []} />
        </Collapsible>
        <Collapsible open={resetOpen} id={resetId} ariaLabel={t("clientConfig.reset.nativeTitle")}>
          <ClientConfigurationResetPanel app={props.app} busy={props.busy} configuration={configuration} />
        </Collapsible>
        <ClientSettingsForm panel={props} directoryOpen={directoryOpen} reviewOpen={reviewOpen} resetOpen={resetOpen} configuration={configuration} />
      </div>
    </div>
    <ClientConfigurationConfirmation
      app={props.app}
      busy={props.busy}
      applying={configuration.applying}
      pending={configuration.pendingApply}
      onConfirm={configuration.commitApply}
      onCancel={configuration.cancelApply}
    />
  </>;
}
