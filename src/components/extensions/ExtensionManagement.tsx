import { useId, useState } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { ExportIcon, TrashIcon, UpdateIcon } from "../icons";
import { SettingsGroup, SettingsLayout, SettingsRow } from "../SettingsLayout";
import { Table } from "../Table";
import { Time } from "../Time";
import { CapabilityPanel } from "./CapabilityPanel";
import { ExtensionBindings, ExtensionInstallTargets } from "./ExtensionBindings";
import { ExtensionCheckPanel } from "./ExtensionCheckPanel";
import { ExtensionResourceFacts } from "./ExtensionResourceFacts";
import { TRANSPORT_LABELS } from "./labels";
import type { ExtensionManagementProps } from "./management-types";
import { SkillSourceLink } from "./SkillSourceCandidate";

function McpUpdateManagement({ item, registryUpdate: report, busy, onApplyUpdate, onEditUpdateConfig }: ExtensionManagementProps) {
  const { t } = useI18n();
  const registry = item.mcpMetadata?.registry;
  if (item.kind !== "mcp" || !registry || !report) return null;
  if (report.status === "failed") {
    return <SettingsRow label={t("extensions.management.sourceUpdates")} wide>
      <p className="asb-warn-text" role="alert">{t("extensions.management.checkFailed", { error: report.message })}</p>
    </SettingsRow>;
  }
  const status = report.status === "current" ? t("extensions.management.upToDate")
    : report.status === "manual" ? t("extensions.registry.manualUpdate", { version: report.version })
    : t("extensions.registry.versionChange", { from: registry.version, to: report.source.version });
  return (
    <SettingsRow label={t("extensions.management.sourceUpdates")} description={status}
      details={report.status === "manual" && report.connections.length > 0 && (
        <>
          <Table rows={report.connections} rowKey={(connection) => JSON.stringify(connection)}
            ariaLabel={t("extensions.management.sourceUpdates")} columns={[
              { key: "transport", header: t("extensions.facts.transport"), render: (row) => t(TRANSPORT_LABELS[row.transport]) },
              { key: "endpoint", header: t("extensions.registry.nextEndpoint"), render: (row) => <code>{row.endpoint}</code> },
              { key: "headers", header: t("extensions.registry.requiredHeaders"), render: (row) => row.requiredHeaders.join(", ") || "—" },
            ]} />
          <p className="asb-scope-note">{t("extensions.registry.manualAssociable")}</p>
        </>
      )}>
      {report.status === "available" && (
        <Button variant="secondary" disabled={busy} onClick={onApplyUpdate}>
          {t(report.action === "associateCurrent" ? "extensions.registry.associateVersion" : "extensions.management.applyUpdate")}
        </Button>
      )}
      {report.status === "manual" && <>
        <SkillSourceLink url={report.docs} label={t("extensions.registry.docs", { name: item.name })} />
        <Button variant="secondary" disabled={busy} onClick={onEditUpdateConfig}>{t("extensions.registry.editConnection")}</Button>
      </>}
    </SettingsRow>
  );
}

function SkillUpdateManagement({ updateReport: report, busy, onApplyUpdate }: ExtensionManagementProps) {
  const { t } = useI18n();
  if (!report) return null;
  if (report.status === "failed") {
    return <SettingsRow label={t("extensions.management.sourceUpdates")} wide>
      <p className="asb-warn-text" role="alert">{t("extensions.management.checkFailed", { error: report.message })}</p>
    </SettingsRow>;
  }
  const status = report.status === "available"
    ? t("extensions.management.newVersionCommit", { commit: report.commit.slice(0, 12) })
    : t("extensions.management.upToDate");
  return (
    <SettingsRow label={t("extensions.management.sourceUpdates")} description={status}>
      {report.status === "available" && (
        <Button variant="secondary" disabled={busy} onClick={onApplyUpdate}>{t("extensions.management.applyUpdate")}</Button>
      )}
    </SettingsRow>
  );
}

function ExtensionVersions(props: ExtensionManagementProps) {
  const { t } = useI18n();
  const { item, busy, onCheckUpdates, onExportPortable } = props;
  const registry = item.kind === "mcp" ? item.mcpMetadata?.registry : null;
  const skillSource = item.kind === "skill" ? item.source : null;
  const canCheck = Boolean(skillSource?.resolvedCommit || registry);
  const source = registry?.name ?? skillSource?.repo
    ?? t(skillSource ? "extensions.facts.sourceRecorded" : "extensions.list.sourceLocal");
  const version = item.kind === "skill" ? item.contentDigest.slice(0, 12) : registry?.version;
  return (
    <SettingsGroup title={t("extensions.management.sourceAndVersion")} actions={(canCheck || onExportPortable) && (
      <div className="asb-ext-actions">
        {canCheck && (
          <Button variant="secondary" disabled={busy} onClick={onCheckUpdates}>
            <UpdateIcon />{t("extensions.management.checkUpdates")}
          </Button>
        )}
        {onExportPortable && (
          <Button variant="secondary" disabled={busy} onClick={onExportPortable}>
            <ExportIcon />{t("extensions.management.exportPortable")}
          </Button>
        )}
      </div>
    )}>
      <SettingsRow label={t("extensions.facts.source")} description={source} children={null}
        details={skillSource?.resolvedCommit && (
          <span className="asb-scope-note">
            {t("extensions.facts.sourceCommit", { commit: skillSource.resolvedCommit.slice(0, 12) })}
          </span>
        )} />
      <SettingsRow label={t("extensions.table.version")} description={
        <span>{t("extensions.management.revision", { revision: item.revision })} · {t("extensions.management.updatedAt")} <Time iso={item.updatedAt} /></span>
      }>
        {version && <code className="asb-code">{version}</code>}
      </SettingsRow>
      {item.kind === "skill" ? <SkillUpdateManagement {...props} /> : <McpUpdateManagement {...props} />}
    </SettingsGroup>
  );
}

export function ExtensionManagement(props: ExtensionManagementProps) {
  const { t } = useI18n();
  const detailsId = useId();
  const [factsOpen, setFactsOpen] = useState(false);
  const { item, capabilities } = props;
  return (
    <SettingsLayout className="asb-ext-management" role="region" aria-label={t("extensions.management.aria", { name: item.name })}>
      <ExtensionBindings {...props} />
      <ExtensionInstallTargets {...props} />
      {item.kind === "mcp" && <ExtensionCheckPanel {...props} />}
      <ExtensionVersions {...props} />
      <SettingsGroup title={t("extensions.management.details")} surface="plain" actions={
        <CollapsibleTrigger open={factsOpen} onOpenChange={setFactsOpen} contentId={detailsId}
          label={t(factsOpen ? "extensions.management.collapseDetails" : "extensions.management.expandDetails")} />
      }>
        <Collapsible open={factsOpen} id={detailsId}>
          <div className="asb-ext-details-content">
            <div><ExtensionResourceFacts item={item} /></div>
            <CapabilityPanel reports={capabilities} />
          </div>
        </Collapsible>
      </SettingsGroup>
      <div className="asb-ext-management-delete">
        <Button variant="dangerText" disabled={props.busy} onClick={props.onDelete}>
          <TrashIcon />{t("extensions.management.deleteDefinition")}
        </Button>
      </div>
    </SettingsLayout>
  );
}
