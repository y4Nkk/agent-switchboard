import { useId, type ReactNode } from "react";
import type { AppSettings, UpdateChannel, UpdateCheck } from "../api/client";
import appIcon from "../assets/app-icon.svg";
import { SETTINGS_SECTIONS, type SettingsSection } from "../app/navigation";
import type { UpdateDownloadProgress } from "../app/useUpdateCheck";
import { useI18n } from "../i18n";
import { AppSettingsForm } from "../components/AppSettingsForm";
import { Button } from "../components/Button";
import { ModuleHeader, WorkspaceHeader } from "../components/WorkspaceHeader";
import { Tabs } from "../components/Tabs";
import { UpdateSection } from "../components/UpdateSection";

interface SettingsPageProps {
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
  /** Replaces an invalid settings file with defaults. */
  onRepair: () => void;
  busy: boolean;
  /** Saves one field of the currently loaded settings. */
  onPatch: (patch: Partial<AppSettings>) => Promise<boolean>;
  /** Restarts the desktop process on the user's explicit request. */
  onRestart: () => void;
  /** Latest manual update check; null until the first check runs. */
  updateCheck: UpdateCheck | null;
  updateChannel: UpdateChannel | null;
  /** Running build's version; null until the process reports it. */
  appVersion: string | null;
  /** A startup or user-triggered release lookup is currently in flight. */
  updateChecking: boolean;
  updateInstalling: boolean;
  updateProgress: UpdateDownloadProgress | null;
  updateCheckedAt: string | null;
  updateRestartRequired: boolean;
  onCheckUpdate: () => void;
  onInstallUpdate: () => void;
  onRestartInstalledUpdate: () => void;
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
          <div className="asb-app-settings-danger-zone">
            <Button variant="danger" disabled={busy} onClick={props.onRepair}>{t("shell.banner.repair")}</Button>
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
      <UpdateSection channel={props.updateChannel} appVersion={props.appVersion}
        result={props.updateCheck} busy={props.busy || props.updateChecking} installing={props.updateInstalling}
        progress={props.updateProgress} checkedAt={props.updateCheckedAt} restartRequired={props.updateRestartRequired}
        onCheck={props.onCheckUpdate} onInstall={props.onInstallUpdate} onRestart={props.onRestartInstalledUpdate} />
    </div>
  );
}

function SettingsPanel({ section, selected, id, labelledBy, children }: {
  section: SettingsSection;
  selected: SettingsSection;
  id: string;
  labelledBy: string;
  children: ReactNode;
}) {
  const { t } = useI18n();
  const title = t(SETTINGS_SECTIONS.find((item) => item.value === section)!.labelKey);
  return (
    <section className="asb-panel" id={id} role="tabpanel" aria-labelledby={labelledBy} hidden={selected !== section}>
      <ModuleHeader title={title} />
      {children}
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
      <SettingsPanel section="application" selected={section}
        id={`${id}-application-panel`} labelledBy={`${id}-application-tab`}>
        <ApplicationSettings {...props} />
      </SettingsPanel>
        <div id={`${id}-client-management-panel`} role="tabpanel" aria-labelledby={`${id}-client-management-tab`} hidden={section !== "client-management"}>
          {props.clientManagement}
        </div>
      <div id={`${id}-backups-panel`} role="tabpanel" aria-labelledby={`${id}-backups-tab`} hidden={section !== "backups"}>
          {props.backups}
        </div>
      <div id={`${id}-storage-panel`} role="tabpanel" aria-labelledby={`${id}-storage-tab`} hidden={section !== "storage"}>
        {props.storage}
      </div>
      <div id={`${id}-diagnostics-panel`} role="tabpanel" aria-labelledby={`${id}-diagnostics-tab`} hidden={section !== "diagnostics"}>
        {props.diagnostics}
      </div>
      <SettingsPanel section="about" selected={section}
        id={`${id}-about-panel`} labelledBy={`${id}-about-tab`}>
        <AboutSettings {...props} />
      </SettingsPanel>
    </section>
  );
}
