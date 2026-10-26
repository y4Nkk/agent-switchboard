import { useState, type ReactNode } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { CollapsibleSection } from "../Collapsible";
import { ExportIcon, PreviewIcon, TrashIcon, UpdateIcon } from "../icons";
import { Time } from "../Time";
import { Table } from "../Table";
import { Tooltip } from "../Tooltip";
import { CapabilityPanel } from "./CapabilityPanel";
import { ExtensionBindings, ExtensionInstallTargets } from "./ExtensionBindings";
import { ExtensionCheckPanel } from "./ExtensionCheckPanel";
import { ExtensionResourceFacts } from "./ExtensionResourceFacts";
import type { ExtensionManagementProps } from "./management-types";
import { SKILL_CHANGE_LABELS } from "./labels";

function ManagementAction({ label, disabled, danger = false, onClick, children }: {
  label: string;
  disabled: boolean;
  danger?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip label={label}>
      <Button
        variant={danger ? "dangerIcon" : "icon"}
        size="compact"
        className={danger ? "asb-ext-danger-action" : undefined}
        disabled={disabled}
        aria-label={label}
        onClick={onClick}
      >
        {children}
      </Button>
    </Tooltip>
  );
}

function ManagementActions(props: ExtensionManagementProps) {
  const { t } = useI18n();
  const { item, busy } = props;
  return (
    <div className="asb-ext-management-actions" role="group" aria-label={t("extensions.management.actionsAria", { name: item.name })}>
      {item.kind === "skill" && (
        <ManagementAction label={t("extensions.management.checkUpdates")} disabled={busy} onClick={props.onCheckUpdates}>
          <UpdateIcon />
        </ManagementAction>
      )}
      {props.onExportPortable && (
        <ManagementAction label={t("extensions.management.exportPortable")} disabled={busy} onClick={props.onExportPortable}>
          <ExportIcon />
        </ManagementAction>
      )}
      {item.bindings.some(
        (binding) => binding.desired === "enabled" && binding.fileState === "pendingApply",
      ) && (
        <ManagementAction label={t("extensions.management.deployCurrent")} disabled={busy} onClick={props.onDeployCurrent}>
          <PreviewIcon />
        </ManagementAction>
      )}
      <ManagementAction label={t("extensions.management.deleteDefinition")} disabled={busy} danger onClick={props.onDelete}>
        <TrashIcon />
      </ManagementAction>
    </div>
  );
}

function SkillUpdateManagement({ item, updateReport: report, busy, onApplyUpdate }: ExtensionManagementProps) {
  const { t } = useI18n();
  if (item.kind !== "skill" || !report) return null;
  if (report.error)
    return (
      <p className="asb-warn-text" role="alert">
        {t("extensions.management.checkFailed", { error: report.error })}
      </p>
    );
  const current = report.upToDate || report.newDigest === item.contentDigest;
  return (
    <div className="asb-ext-section">
      <div className="asb-ext-section-heading">
        <h4 className="asb-section-title">{t("extensions.management.sourceUpdates")}</h4>
        <span className="asb-scope-note">
          {current
            ? t("extensions.management.upToDate")
            : report.newCommit
              ? t("extensions.management.newVersionCommit", { commit: report.newCommit.slice(0, 12) })
              : t("extensions.management.newVersion")}
        </span>
        {!current && (
          <Button variant="secondary" disabled={busy || !report.newDigest} onClick={onApplyUpdate}>
            {t("extensions.management.applyUpdate")}
          </Button>
        )}
      </div>
      {!current && report.changedFiles.length > 0 && (
        <Table rows={report.changedFiles} rowKey={(file) => `${file.action}:${file.relativePath}`}
          ariaLabel={t("extensions.management.sourceUpdates")} columns={[
            { key: "file", header: t("extensions.table.file"), render: (file) => <span className="asb-code">{file.relativePath}</span> },
            { key: "change", header: t("extensions.table.change"), render: (file) => t(SKILL_CHANGE_LABELS[file.action]) },
          ]} />
      )}
    </div>
  );
}

export function ExtensionManagement(props: ExtensionManagementProps) {
  const { t } = useI18n();
  const [factsOpen, setFactsOpen] = useState(false);
  const { item, capabilities } = props;
  return (
    <section className="asb-ext-management" aria-label={t("extensions.management.aria", { name: item.name })}>
      <header className="asb-ext-management-head">
        <p className="asb-scope-note">
          {item.kind === "mcp" ? t("extensions.kind.mcp") : t("extensions.kind.skill")} · {t("extensions.management.updatedAt")} <Time iso={item.updatedAt} /> · {t("extensions.management.revision", { revision: item.revision })}
        </p>
        <ManagementActions {...props} />
      </header>
      <div className="asb-ext-management-main">
        <ExtensionBindings {...props} />
        <ExtensionInstallTargets {...props} />
        {item.kind === "mcp" && (
          <ExtensionCheckPanel {...props} />
        )}
        <SkillUpdateManagement {...props} />
        <CollapsibleSection quiet open={factsOpen} onOpenChange={setFactsOpen}
          label={t("extensions.management.details")}>
          <ExtensionResourceFacts item={item} />
          <CapabilityPanel reports={capabilities} />
        </CollapsibleSection>
      </div>
    </section>
  );
}
