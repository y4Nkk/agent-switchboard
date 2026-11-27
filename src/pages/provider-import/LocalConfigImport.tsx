import { FileText, Link2, LockKeyhole, Plug, Server, ShieldCheck, TriangleAlert } from "lucide-react";
import { openConfigFileLocation, type AppKind, type ClaudeImportProposal, type CodexImportProposal, type DiscoveredFile, type DiscoveryReport } from "../../api/client";
import { Button } from "../../components/Button";
import { FactList, FactStatus, type FactListRow } from "../../components/FactList";
import { FactPath } from "../../components/FactPath";
import { ModuleHeader } from "../../components/WorkspaceHeader";
import { SearchIcon } from "../../components/icons";
import { useI18n, type TFunction } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { clientName } from "../../lib/client-name";

interface LocalConfigImportProps {
  app: AppKind;
  discovery: DiscoveryReport | null;
  busy: boolean;
  onScan: () => void;
  onImport: () => void;
}

function stateLabel(file: DiscoveredFile, t: TFunction): string {
  switch (file.state.kind) {
    case "ok": return t("importDiscovery.local.state.ok");
    case "missing": return t("importDiscovery.local.state.missing");
    case "readError": return t("importDiscovery.local.state.readError");
    case "parseError": return t("importDiscovery.local.state.parseError");
  }
}

function localRouteFacts(file: DiscoveredFile, t: TFunction): FactListRow[] {
  if (file.state.kind !== "ok") return [];
  const { route, managed, warnings, importable } = file.state;
  const rows: FactListRow[] = [{ label: t("importDiscovery.local.currentService"), icon: Plug,
    value: `${route.routeMode === "official" ? t("importDiscovery.label.official") : t("importDiscovery.local.customService")} · ${route.model ?? t("importDiscovery.local.defaultModel")}` }];
  if (route.providerName) rows.push({ label: t("importDiscovery.label.provider"), icon: Server, value: route.providerName });
  if (route.baseUrl) rows.push({ label: t("importDiscovery.local.serviceUrl"), icon: Link2, format: "code", value: route.baseUrl });
  if (route.apiKey) rows.push({ label: t("importDiscovery.local.credentialVar"), icon: LockKeyhole, format: "code", value: route.apiKey });
  rows.push({ label: t("importDiscovery.local.managedState"), icon: ShieldCheck,
    value: <FactStatus tone={managed ? "success" : "neutral"}>
      {t(managed ? "importDiscovery.local.managed" : "importDiscovery.local.unmanaged")}
    </FactStatus> });
  if (warnings.length > 0 || (!importable && !managed)) rows.push({
    label: t("importDiscovery.local.warnings"), icon: TriangleAlert, tone: "warning",
    value: <>
      {warnings.map((warning) => <span key={warning.key} className="asb-warn-text asb-status-warn">{localizedMessageText(warning, t)}</span>)}
      {!importable && !managed && <span className="asb-warn-text asb-status-warn">{t("importDiscovery.local.unimportable")}</span>}
    </> });
  return rows;
}

function LocalConfigCard({ file, proposal, busy, onImport }: {
  file: DiscoveredFile;
  proposal: ClaudeImportProposal | CodexImportProposal | undefined;
  busy: boolean;
  onImport: () => void;
}) {
  const { t } = useI18n();
  return (
    <article className="asb-client-status" aria-label={t("importDiscovery.local.cardAria", { client: clientName(file.app) })}>
      <header className="asb-client-status-head">
        <span className="asb-client-status-name">{clientName(file.app)}</span>
        <span className={`asb-status-pill${file.state.kind === "ok" ? " is-ok" : ""}`}>
          <span className="asb-status-pill-dot" aria-hidden="true" />{stateLabel(file, t)}
        </span>
      </header>
      <FactList rows={[
        { label: t("importDiscovery.local.configFile"), icon: FileText,
          value: <FactPath path={file.path} open={() => openConfigFileLocation(file.app)} /> },
        ...(file.state.kind === "readError" ? [{ label: t("importDiscovery.local.errorDetail"), icon: TriangleAlert, tone: "warning" as const,
          value: <span className="asb-warn-text">{localizedMessageText(file.state.message, t)}</span> }] : []),
        ...(file.state.kind === "parseError" ? [{ label: t("importDiscovery.local.errorDetail"), icon: TriangleAlert, tone: "warning" as const,
          value: <span className="asb-warn-text">
            {file.state.line !== null ? t("importDiscovery.local.errorLine", { line: file.state.line }) : ""}{file.state.message}
          </span> }] : []),
        ...localRouteFacts(file, t),
      ]} />
      {proposal && <div className="asb-discovery-import">
        <p className="asb-discovery-basis">{localizedMessageText(proposal.basis, t)}</p>
        <Button variant="secondary" disabled={busy} onClick={onImport}>{t("importDiscovery.local.importProvider")}</Button>
      </div>}
    </article>
  );
}

export function LocalConfigImport({ app, discovery, busy, onScan, onImport }: LocalConfigImportProps) {
  const { t } = useI18n();
  return (
    <>
      <ModuleHeader
        title={t("importDiscovery.tab.local")}
        primaryActions={
          <Button variant="secondary" disabled={busy} onClick={onScan}>{discovery ? t("importDiscovery.local.scanRefresh") : t("importDiscovery.local.scan")}</Button>
        }
      />
      {discovery ? <LocalConfigCard file={discovery[app]} busy={busy} onImport={onImport}
        proposal={app === "claude" ? discovery.claudeImportProposals[0] : discovery.codexImportProposals[0]} />
        : (
          <div className="asb-empty-state">
            <span className="asb-empty-state-icon" aria-hidden="true">
              <SearchIcon />
            </span>
            <h3 className="asb-section-title">{t("importDiscovery.local.notScanned", { client: clientName(app) })}</h3>
          </div>
        )}
    </>
  );
}
