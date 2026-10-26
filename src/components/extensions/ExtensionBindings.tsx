import { useState } from "react";
import type { BindingStatus } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { CollapsibleSection } from "../Collapsible";
import { ConnectivityIcon, PinIcon, TrashIcon } from "../icons";
import { Tooltip } from "../Tooltip";
import { Checkbox } from "../Checkbox";
import { Table, type TableColumn } from "../Table";
import type { ExtensionManagementProps } from "./management-types";
import { FILE_STATE_LABELS, targetLabel, targetValue } from "./labels";
import { extensionTargetOptions } from "./targets";

function BindingActions({ binding, props }: { binding: BindingStatus; props: ExtensionManagementProps }) {
  const { t } = useI18n();
  const target = targetLabel(binding.target, props.projectNames);
  return <div className="asb-ext-binding-actions">
    {props.item.kind === "skill" && (
      <Tooltip label={t(binding.lockedDigest ? "extensions.bindings.unpin" : "extensions.bindings.pin")}>
        <Button variant="icon" size="compact"
          disabled={props.busy || (!binding.lockedDigest && binding.fileState !== "inSync")}
          aria-label={`${t(binding.lockedDigest ? "extensions.bindings.unpinAction" : "extensions.bindings.pinAction")} ${target}`}
          onClick={() => props.onToggleLock(binding, binding.lockedDigest == null)}><PinIcon /></Button>
      </Tooltip>
    )}
    <Tooltip label={t("extensions.bindings.remove")}>
      <Button variant="dangerIcon" size="compact" className="asb-ext-danger-action" disabled={props.busy}
        aria-label={`${t("extensions.bindings.remove")} ${target}`}
        onClick={() => props.onRemoveBinding(binding)}><TrashIcon /></Button>
    </Tooltip>
  </div>;
}

export function ExtensionBindings(props: ExtensionManagementProps) {
  const { t } = useI18n();
  const columns: TableColumn<BindingStatus>[] = [
    { key: "target", header: t("extensions.table.target"), render: (binding) => (
      <Checkbox checked={binding.desired === "enabled"} label={targetLabel(binding.target, props.projectNames)}
        ariaLabel={`${t(binding.desired === "enabled" ? "extensions.bindings.disableAction" : "extensions.bindings.enableAction")} ${targetLabel(binding.target, props.projectNames)}`}
        disabled={props.busy} onChange={(checked) => props.onChangeBinding(binding, checked)} />
    ) },
    { key: "status", header: t("extensions.table.status"), render: (binding) => (
      <div className="asb-ext-table-cell-stack">
        <span className="asb-pill-status">{t(FILE_STATE_LABELS[binding.fileState])}</span>
        {binding.desired === "disabled" && <span className="asb-pill-status">{t("extensions.bindings.disabled")}</span>}
        {binding.warnings.map((warning) => <span key={warning} className="asb-warn-text">{warning}</span>)}
      </div>
    ) },
  ];
  if (props.item.kind === "skill") columns.push({
    key: "version", header: t("extensions.table.version"), render: (binding) => binding.lockedDigest
      ? <span className="asb-pill-status">{t("extensions.bindings.pinned", { digest: binding.lockedDigest.slice(0, 12) })}</span>
      : <span className="asb-scope-note">{t("extensions.bindings.followCurrent")}</span>,
  });
  columns.push({ key: "actions", header: t("extensions.table.actions"),
    render: (binding) => <BindingActions binding={binding} props={props} /> });
  return <section className="asb-ext-section">
    <h4 className="asb-section-title">{t("extensions.bindings.title")}</h4>
    {props.item.bindings.length === 0 ? (
      <div className="asb-empty-state">
        <span className="asb-empty-state-icon" aria-hidden="true"><ConnectivityIcon /></span>
        <h3 className="asb-section-title">{t("extensions.bindings.empty")}</h3>
      </div>
    ) : <Table columns={columns} rows={props.item.bindings} rowKey={(binding) => binding.id}
      ariaLabel={t("extensions.bindings.title")} className="asb-ext-management-table" />}
  </section>;
}

export function ExtensionInstallTargets({
  item, projects, projectNames, busy, installTargets, onInstallTargetsChange, onInstall,
}: ExtensionManagementProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const existing = new Set(item.bindings.map((binding) => targetValue(binding.target)));
  const options = extensionTargetOptions(item, projects).filter((option) => !existing.has(option.value));
  if (options.length === 0) return null;
  return <section className="asb-ext-section">
    <CollapsibleSection className="asb-ext-install-targets" open={open} onOpenChange={setOpen}
      label={<span className="asb-section-title">{t("extensions.install.title")}</span>}
      value={t("extensions.install.count", { count: options.length })}>
      <Table rows={options} rowKey={(option) => option.value} ariaLabel={t("extensions.install.listAria")}
        className="asb-ext-management-table" columns={[
          { key: "client", header: t("extensions.table.client"), render: (option) => (
            <Checkbox checked={installTargets.includes(option.value)} label={clientName(option.target.client)}
              ariaLabel={`${t("extensions.install.listAria")} ${option.label}`} disabled={busy}
              onChange={(checked) => onInstallTargetsChange(checked
                ? [...installTargets, option.value] : installTargets.filter((value) => value !== option.value))} />
          ) },
          { key: "scope", header: t("extensions.table.scope"), render: (option) => t(option.target.scope === "app"
            ? "extensions.target.app" : option.target.scope === "projectShared"
              ? "extensions.target.projectShared" : "extensions.target.projectPrivate") },
          { key: "project", header: t("extensions.table.project"), render: (option) => option.target.scope === "app"
            ? "—" : projectNames.get(option.target.projectId) ?? option.target.projectId },
        ]} />
      <Button variant="primary" disabled={busy || installTargets.length === 0} onClick={onInstall}>
        {t("extensions.install.selected")}
      </Button>
    </CollapsibleSection>
  </section>;
}
