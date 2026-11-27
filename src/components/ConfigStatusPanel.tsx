import { Clock3, FileText, Link2, LockKeyhole, Server, TriangleAlert } from "lucide-react";
import { openConfigFileLocation, type AppKind, type ConfigFileStatus, type LockStatus, type ProviderProfile } from "../api/client";
import { clientName } from "../lib/client-name";
import { currentProviderName } from "../lib/current-provider-name";
import { useI18n, type TFunction } from "../i18n";
import { localizedMessageText } from "../i18n/errors";
import { Button } from "./Button";
import { ClientLogo } from "./ClientLogo";
import { FactList, FactStatus, type FactListRow } from "./FactList";
import { FactPath } from "./FactPath";
import { Time } from "./Time";
import { ModuleHeader } from "./WorkspaceHeader";

export interface ConfigStatusPanelProps {
  statuses: ConfigFileStatus[] | null;
  profiles: ProviderProfile[];
  locks: Partial<Record<AppKind, LockStatus>>;
  busy: boolean;
  onRecoverLock: (app: AppKind) => void;
}

function lockLabel(t: TFunction, status: LockStatus | undefined): string {
  if (!status) return t("clientConfig.lock.loading");
  switch (status.state) {
    case "free":
      return t("clientConfig.lock.free");
    case "held": {
      const holder = status.processName ?? (status.pid ? t("clientConfig.lock.process", { pid: status.pid }) : t("clientConfig.lock.otherProcess"));
      return t("clientConfig.lock.held", { holder });
    }
    case "stale":
      return t("clientConfig.lock.stale");
    case "indeterminate":
      return t("clientConfig.lock.indeterminate", { reason: status.reason });
  }
}

function statusPill(t: TFunction, status: ConfigFileStatus): { ok: boolean; text: string } {
  if (status.recoveryIssue) return { ok: false, text: t("clientConfig.pill.recoveryPending") };
  if (status.readError) return { ok: false, text: t("clientConfig.pill.readFailed") };
  if (!status.exists) return { ok: false, text: t("clientConfig.pill.missing") };
  if (!status.syntaxOk) return { ok: false, text: t("clientConfig.pill.syntaxError") };
  if (status.clientSettingsError) return { ok: false, text: t("clientConfig.pill.settingsError") };
  return { ok: true, text: t("clientConfig.pill.ok") };
}

function ConfigStatusDetails({ status, profiles, lock }: {
  status: ConfigFileStatus;
  profiles: ProviderProfile[];
  lock: LockStatus | undefined;
}) {
  const { t } = useI18n();
  const readable = !status.readError && status.exists && status.syntaxOk;
  const rows: FactListRow[] = [
    { label: t("clientConfig.details.configFile"), icon: FileText,
      value: <FactPath path={status.path} open={() => openConfigFileLocation(status.app)} /> },
  ];
  if (status.recoveryIssue) rows.push({ label: t("clientConfig.details.recoveryBlocked"), icon: TriangleAlert, tone: "warning",
    value: <><span role="status" className="asb-status-warn">{status.recoveryIssue}</span>
      <span className="asb-status-warn">{t("clientConfig.details.recoveryHelp")}</span></> });
  if (status.readError) rows.push({ label: t("clientConfig.details.readError"), icon: TriangleAlert, tone: "warning",
    value: <span className="asb-warn-text">{status.readError}</span> });
  if (readable) rows.push(
    { label: t("clientConfig.details.currentProvider"), icon: Server,
      value: <>{currentProviderName(status, profiles)} · {status.route?.model ?? t("clientConfig.details.defaultModel")}</> },
  );
  if (readable && status.activeProfileId === null) rows.push({
    label: t("clientConfig.details.profile"), icon: Link2, value: t("providers.route.notes.unlinked"),
  });
  if (readable && status.clientSettingsError) rows.push({
    label: t("clientConfig.details.settingsError"), icon: TriangleAlert, tone: "warning",
    value: <span className="asb-warn-text">{status.clientSettingsError}</span>,
  });
  if (status.lastSwitch) rows.push({ label: t("clientConfig.details.lastWrite"), icon: Clock3,
    value: <>
      <Time iso={status.lastSwitch.at} />
      {status.lastSwitch.operation === "restore"
        ? t("clientConfig.details.lastRestore")
        : status.lastSwitch.operation === "gatewayPortChange"
          ? t("clientConfig.details.lastGatewayPort")
          : status.lastSwitch.profileName
            ? t("clientConfig.details.lastProjected", { name: status.lastSwitch.profileName })
            : t("clientConfig.details.lastWritten")}
    </> });
  if ((status.route?.scopeWarnings.length ?? 0) > 0) rows.push({
    label: t("clientConfig.details.scopeWarnings"), icon: TriangleAlert, tone: "warning",
    value: <>{status.route?.scopeWarnings.map((warning) => <span key={warning.key} className="asb-warn-text asb-status-warn">{localizedMessageText(warning, t)}</span>)}</> });
  const lockTone = lock?.state === "free" ? "success"
    : lock?.state === "stale" || lock?.state === "indeterminate" ? "warning" : "neutral";
  rows.push({ label: t("clientConfig.details.writeLock"), icon: LockKeyhole,
    value: <FactStatus tone={lockTone}>{lockLabel(t, lock)}</FactStatus> });
  return <FactList rows={rows} />;
}

function ConfigStatusCard({ status, profiles, lock, busy, onRecoverLock }: {
  status: ConfigFileStatus;
  profiles: ProviderProfile[];
  lock: LockStatus | undefined;
  busy: boolean;
  onRecoverLock: (app: AppKind) => void;
}) {
  const { t } = useI18n();
  const pill = statusPill(t, status);
  return (
    <article className="asb-client-status" aria-label={t("clientConfig.details.statusAria", { name: clientName(status.app) })}>
      <header className="asb-client-status-head">
        <span className="asb-client-status-name"><ClientLogo app={status.app} className="asb-status-logo" />{clientName(status.app)}</span>
        <span className={`asb-status-pill${pill.ok ? " is-ok" : ""}`}>
          <span className="asb-status-pill-dot" aria-hidden="true" />{pill.text}
        </span>
      </header>
      <ConfigStatusDetails status={status} profiles={profiles} lock={lock} />
      {lock?.state === "stale" && (
        <div className="asb-kv-actions">
          <Button variant="secondary" disabled={busy} onClick={() => onRecoverLock(status.app)}>{t("clientConfig.details.recoverLock")}</Button>
        </div>
      )}
    </article>
  );
}

export function ConfigStatusPanel({ statuses, profiles, locks, busy, onRecoverLock }: ConfigStatusPanelProps) {
  const { t } = useI18n();
  return (
    <section className="asb-panel" aria-labelledby="configuration-status-heading">
      <ModuleHeader id="configuration-status-heading" title={t("clientConfig.statusPanel.title")} />
      {statuses === null ? (
        /* Two client sections at the real anatomy's footprint: a head line and
           five fact lines each, so the panel does not reflow on arrival. */
        <div role="status" aria-label={t("clientConfig.statusPanel.loading")}>
          {Array.from({ length: 2 }, (_, section) => (
            <div key={section} className="asb-client-status asb-client-status-skeleton" aria-hidden="true">
              {Array.from({ length: 6 }, (_, line) => (
                <span key={line} className="asb-skeleton" />
              ))}
            </div>
          ))}
        </div>
      ) : (
        statuses.map((status) => (
          <ConfigStatusCard key={status.app} status={status} profiles={profiles} lock={locks[status.app]}
            busy={busy} onRecoverLock={onRecoverLock} />
        ))
      )}
    </section>
  );
}
