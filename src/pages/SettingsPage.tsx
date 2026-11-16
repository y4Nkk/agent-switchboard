import { useId, type ReactNode } from "react";
import type { AppSettings } from "../api/client";
import appIcon from "../assets/app-icon.svg";
import { SETTINGS_SECTIONS, type SettingsSection } from "../app/navigation";
import type { ApplicationUpdateControl } from "../app/update-state";
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
  onSectionChange: (section: SettingsSection) => void;
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
        <img className="asb-about-logo" src={appIcon} alt="" />
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

function SettingsPanel({ section, selected, active, id, labelledBy, title, children }: {
  section: SettingsSection;
  selected: SettingsSection;
  active: boolean;
  id: string;
  labelledBy: string;
  title?: string;
  children: ReactNode;
}) {
  return (
    <section className={title ? "asb-panel" : undefined} id={id} role="tabpanel" aria-labelledby={labelledBy} hidden={selected !== section}>
      <PageErrorBoundary active={active && selected === section}>
        {title && <ModuleHeader title={title} />}
        {children}
      </PageErrorBoundary>
    </section>
  );
}

export function SettingsPage(props: SettingsPageProps) {
  const { section, onSectionChange } = props;
  const { t } = useI18n();
  const id = useId();
  return (
    <section className="asb-page-stack asb-settings-content" aria-label={t("nav.page.settings")}>
      <WorkspaceHeader title={t("nav.page.settings")} primary={
        <Tabs value={section} onChange={onSectionChange} scope={id} label={t("settings.page.categories.aria")}
          tabs={SETTINGS_SECTIONS.map((tab) => ({
            value: tab.value, label: t(tab.labelKey), controls: `${id}-${tab.value}-panel`,
          }))} />
      } primaryActions={props.onReturnToProviders ? (
        <Button variant="secondary" onClick={props.onReturnToProviders}>{t("settings.page.backToProviders")}</Button>
      ) : undefined} />
      <SettingsPanel section="application" selected={section} active={props.active} title={t("nav.section.application")}
        id={`${id}-application-panel`} labelledBy={`${id}-application-tab`}>
        <ApplicationSettings {...props} />
      </SettingsPanel>
      <SettingsPanel section="client-management" selected={section} active={props.active}
        id={`${id}-client-management-panel`} labelledBy={`${id}-client-management-tab`}>
        {props.clientManagement}
      </SettingsPanel>
      <SettingsPanel section="backups" selected={section} active={props.active}
        id={`${id}-backups-panel`} labelledBy={`${id}-backups-tab`}>
        {props.backups}
      </SettingsPanel>
      <SettingsPanel section="storage" selected={section} active={props.active}
        id={`${id}-storage-panel`} labelledBy={`${id}-storage-tab`}>
        {props.storage}
      </SettingsPanel>
      <SettingsPanel section="diagnostics" selected={section} active={props.active}
        id={`${id}-diagnostics-panel`} labelledBy={`${id}-diagnostics-tab`}>
        {props.diagnostics}
      </SettingsPanel>
      <SettingsPanel section="about" selected={section} active={props.active} title={t("nav.section.about")}
        id={`${id}-about-panel`} labelledBy={`${id}-about-tab`}>
        <AboutSettings {...props} />
      </SettingsPanel>
    </section>
  );
}
