import type { ExtensionListItem, McpCheckOutcome, SkillUpdate } from "../../api/client";
import { Button } from "../../components/Button";
import { UpdateIcon } from "../../components/icons";
import { Table, type TableColumn } from "../../components/Table";
import { Tooltip } from "../../components/Tooltip";
import { ClientToggleGroup } from "../../components/extensions/ClientToggleGroup";
import { MANAGEMENT_CLIENTS } from "../../components/extensions/client-presentation";
import { TRANSPORT_LABELS } from "../../components/extensions/labels";
import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n";
import { tr } from "../../i18n/current";
import { pendingDeployment } from "./pending-deployment";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

const MCP_CHECK_STATES: Record<McpCheckOutcome["kind"], { tone?: "safe" | "warning" | "danger"; key: MessageKey }> = {
  passed: { tone: "safe", key: "extensions.list.checkPassed" },
  partial: { tone: "warning", key: "extensions.list.checkPartial" },
  failed: { tone: "danger", key: "extensions.list.checkFailed" },
  cancelled: { key: "extensions.list.checkCancelled" },
  needsNativeConfirmation: { key: "extensions.list.checkNativeConfirmation" },
};

function rowDescription(item: ExtensionListItem): { tone: string | null; text: string } {
  if (item.kind === "skill") return { tone: null, text: item.manifest.description ?? "" };
  const detail = item.transport === "stdio"
    ? tr("extensions.list.argsDetail", { command: item.command, count: item.argumentCount })
    : "";
  const state = item.checks[0] ? MCP_CHECK_STATES[item.checks[0].outcome.kind] : null;
  return {
    tone: state?.tone ?? null,
    text: [state ? tr(state.key) : tr("extensions.list.neverChecked"), detail].filter(Boolean).join(" · "),
  };
}

function RowActions({
  item,
  report,
  workspace,
}: {
  item: ExtensionListItem;
  report?: SkillUpdate;
  workspace: ExtensionWorkspace;
}) {
  const { t } = useI18n();
  const registry = workspace.registryUpdates.reportMap.get(item.id);
  const associate = registry?.status === "available" && registry.action === "associateCurrent";
  const updateLabel = t(associate ? "extensions.registry.associateVersion" : "extensions.management.applyUpdate");
  return (
    <div className="asb-ext-table-actions">
      <Tooltip label={t("extensions.management.aria", { name: item.name })}>
        <Button
          variant="secondary"
          size="compact"
          disabled={workspace.writeBlocked}
          aria-label={t("extensions.list.openManagementAria", { name: item.name })}
          onClick={() => workspace.nav.openManagement(item.id, item.kind)}
        >
          {t("extensions.list.manage")}
        </Button>
      </Tooltip>
      {(report || registry?.status === "available") && (
        <Button variant="secondary" size="compact" disabled={workspace.writeBlocked}
          aria-label={t("extensions.list.updateAria", { action: updateLabel, name: item.name })}
          onClick={() => {
            if (registry?.status === "available") workspace.nav.setDialog({ type: "mcpRegistryUpdate", report: registry });
            else if (report) void workspace.updates.update([report]);
          }}>
          <UpdateIcon />{updateLabel}
        </Button>
      )}
    </div>
  );
}

function ExtensionIdentity({ item, workspace }: { item: ExtensionListItem; workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const report = workspace.updates.reportMap.get(item.id);
  const registry = workspace.registryUpdates.reportMap.get(item.id);
  const registryVersion = item.mcpMetadata?.registry?.version;
  const failure = report?.status === "failed" ? report.message : registry?.status === "failed" ? registry.message : null;
  const updatable = workspace.updates.updatable.find((entry) => entry.definitionId === item.id);
  const description = rowDescription(item);
  const skillSource = item.kind === "skill"
    ? item.source?.resolvedCommit ? "GitHub" : t("extensions.list.sourceLocal") : null;
  const transport = item.kind === "mcp" ? t(TRANSPORT_LABELS[item.transport]) : null;
  const summary = [transport, description.text].filter(Boolean).join(" · ");
  return (
    <Button
      variant="ghost"
      className="asb-ext-table-definition"
      aria-label={item.kind === "mcp"
        ? t("extensions.list.editMcpAria", { name: item.name })
        : t("extensions.list.editSkillAria", { name: item.name })}
      onClick={() => void workspace.edit(item)}
    >
      <span className="asb-ext-table-title">
        <Tooltip asChild label={item.name}><span className="asb-ext-table-name">{item.name}</span></Tooltip>
        {skillSource && <span className="asb-ext-table-meta">{skillSource}</span>}
        {registryVersion && <span className="asb-ext-table-meta">{registryVersion}</span>}
        {(updatable || registry?.status === "available") && <span className="asb-ext-update-badge">{t("extensions.list.updatable")}</span>}
        {registry?.status === "manual" && <span className="asb-ext-update-badge">{t("extensions.registry.manualUpdate", { version: registry.version })}</span>}
        {failure && <Tooltip asChild label={failure}><span className="asb-ext-table-error">{t("extensions.list.checkError")}</span></Tooltip>}
      </span>
      {summary && <Tooltip asChild label={summary}>
        <span className="asb-ext-table-description">
          {transport && `${transport} · `}
          {description.tone && (
            <span className="asb-ext-check-dot" data-tone={description.tone} aria-hidden="true" />
          )}
          {description.text}
        </span>
      </Tooltip>}
    </Button>
  );
}

function columns(workspace: ExtensionWorkspace): Array<TableColumn<ExtensionListItem>> {
  return [
    {
      key: "definition",
      header: tr("extensions.list.colDefinition"),
      cellClassName: "asb-ext-table-definition-cell",
      render: (item) => <ExtensionIdentity item={item} workspace={workspace} />,
    },
    {
      key: "deployments",
      header: tr("extensions.list.colClients"),
      cellClassName: "asb-ext-table-deployments",
      render: (item) => (
        <ClientToggleGroup
          item={item}
          busy={workspace.writeBlocked}
          pendingClients={MANAGEMENT_CLIENTS.filter((client) =>
            pendingDeployment(workspace.applies.pendingOperations, item, client))}
          onToggle={(client) => void workspace.toggleClient(item, client)}
        />
      ),
    },
    {
      key: "actions",
      header: tr("extensions.list.colActions"),
      cellClassName: "asb-ext-table-action-cell",
      render: (item) => (
        <RowActions
          item={item}
          report={workspace.updates.updatable.find((entry) => entry.definitionId === item.id)}
          workspace={workspace}
        />
      ),
    },
  ];
}

export function ExtensionList({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  return (
    <Table
      scrollMode="viewport"
      className="asb-ext-table"
      columns={columns(workspace)}
      rows={workspace.visible}
      rowKey={(item) => item.id}
      ariaLabel={t("extensions.list.aria")}
    />
  );
}
