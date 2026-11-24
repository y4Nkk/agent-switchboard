import { useId, type ReactNode, type RefCallback } from "react";
import type { AppSettings } from "../api/client";
import { AppLogo } from "../components/AppLogo";
import { SETTINGS_SECTIONS, type SettingsSection } from "../app/navigation";
import type { ApplicationUpdateControl } from "../app/update-state";
import type { usePanelSlideTransition } from "../app/usePanelSlideTransition";
import { useI18n } from "../i18n";
import { AppSettingsForm } from "../components/AppSettingsForm";
import { Button } from "../components/Button";
import { ModuleHeader, WorkspaceHeader } from "../components/WorkspaceHeader";
import { Tabs } from "../components/Tabs";
import { UpdateSection } from "../components/UpdateSection";
import { ChangelogHistory } from "../components/ChangelogHistory";
import { PageErrorBoundary } from "../components/PageErrorBoundary";

interface SettingsPageProps {
  active: boolean;
  section: SettingsSection;
  transition: ReturnType<typeof usePanelSlideTransition<SettingsSection>>;
  onReturnToProviders?: () => void;
  backups: ReactNode;
  clientManagement: ReactNode;
  diagnostics: ReactNode;
  egress: ReactNode;
  storage: ReactNode;
  settings: AppSettings | null;
  /** Why settings could not load; null while loading or after success. */
  loadError: string | null;
  desktopError: string | null;
  /** Re-runs the settings load after a failure. */
  onRetryLoad: () => void;
  busy: boolean;
  /** Saves one field of the currently loaded settings. */
  onPatch: (patch: Partial<AppSettings>) => Promise<boolean>;
  /** Restarts the desktop process on the user's explicit request. */
  onRestart: () => void;
  update: ApplicationUpdateControl;
}

function ApplicationSettings(props: SettingsPageProps) {
  const { settings, busy, onPatch } = props;
  const { t } = useI18n();
  return (
    <div className="asb-app-settings">
      {settings ? (
        <AppSettingsForm settings={settings} busy={busy} onPatch={onPatch}
          desktopError={props.desktopError} onRestart={props.onRestart} />
      ) : props.loadError ? (
        <>
          <div className="asb-app-setting-row" role="alert">
            <div className="asb-app-setting-copy">
              <span className="asb-checkbox-label">{t("settings.loadFailed", { detail: props.loadError })}</span>
              <span className="asb-app-setting-detail">{t("settings.loadFailedDetail")}</span>
            </div>
            <div className="asb-app-settings-error-actions">
              <Button variant="secondary" disabled={busy} onClick={props.onRetryLoad}>{t("settings.retry")}</Button>
            </div>
          </div>

        </>
      ) : <div className="asb-settings-skeleton" role="status" aria-label={t("settings.page.loading")}>
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
      </div>}
      {props.egress}
    </div>
  );
}

function AboutSettings(props: SettingsPageProps) {
  const { t } = useI18n();
  return (
    <div className="asb-app-settings">
      <div className="asb-about-product">
        <AppLogo className="asb-about-logo" />
        <div className="asb-about-copy">
          <h3 className="asb-section-title">Agent Switchboard</h3>
          <p className="asb-field-help">{t("settings.about.tagline")}</p>
        </div>
      </div>
      <UpdateSection update={props.update} busy={props.busy} />
      <ChangelogHistory appVersion={props.update.state.appVersion} />
    </div>
  );
}

function SettingsPanel({ selected, leaving, active, panelRef, id, labelledBy, title, children }: {
  selected: boolean;
  leaving: boolean;
  active: boolean;
  panelRef: RefCallback<HTMLElement>;
  id: string;
  labelledBy: string;
  title?: string;
  children: ReactNode;
}) {
  return (
    <section ref={panelRef} className={title ? "asb-panel" : undefined} id={id} role="tabpanel" aria-labelledby={labelledBy}
      hidden={!selected && !leaving} inert={leaving || undefined} aria-hidden={leaving || undefined}
      data-settings-panel-leaving={leaving || undefined}>
      <PageErrorBoundary active={active && selected}>
        {title && <ModuleHeader title={title} />}
        {children}
      </PageErrorBoundary>
    </section>
  );
}

export function SettingsPage(props: SettingsPageProps) {
  const { section, transition } = props;
  const { t } = useI18n();
  const id = useId();
  const contents: Record<SettingsSection, ReactNode> = {
    application: <ApplicationSettings {...props} />,
    "client-management": props.clientManagement,
    backups: props.backups,
    storage: props.storage,
    diagnostics: props.diagnostics,
    about: <AboutSettings {...props} />,
  };
  return (
    <section className="asb-page-stack asb-settings-content" aria-label={t("nav.page.settings")}>
      <WorkspaceHeader title={t("nav.page.settings")} primary={
        <Tabs value={section} onChange={transition.select} scope={id} label={t("settings.page.categories.aria")}
          tabs={SETTINGS_SECTIONS.map((tab) => ({
            value: tab.value, label: t(tab.labelKey), controls: `${id}-${tab.value}-panel`,
          }))} />
      } primaryActions={props.onReturnToProviders ? (
        <Button variant="secondary" onClick={props.onReturnToProviders}>{t("settings.page.backToProviders")}</Button>
      ) : undefined} />
      <div ref={transition.stageRef} className="asb-settings-panels">
        {SETTINGS_SECTIONS.map(({ value, labelKey }) => (
          <SettingsPanel key={value} selected={section === value} leaving={transition.leaving === value} active={props.active}
            panelRef={transition.panelRefs[value]} id={`${id}-${value}-panel`} labelledBy={`${id}-${value}-tab`}
            title={value === "application" || value === "about" ? t(labelKey) : undefined}>
            {contents[value]}
          </SettingsPanel>
        ))}
      </div>
    </section>
  );
}
