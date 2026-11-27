import type { BindingStatus } from "../../api/client";
import { useI18n } from "../../i18n";
import { commandErrorText } from "../../i18n/errors";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { ConnectivityIcon, PinIcon, PreviewIcon, TrashIcon } from "../icons";
import { Tooltip } from "../Tooltip";
import { Checkbox } from "../Checkbox";
import { Table, type TableColumn } from "../Table";
import { SettingsGroup } from "../SettingsLayout";
import type { ExtensionManagementProps } from "./management-types";
import { FILE_STATE_LABELS, outcomeText, targetLabel, targetValue } from "./labels";
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
        disabled={props.busy} ariaBusy={props.pendingBindings.has(binding.id)} onChange={(checked) => props.onChangeBinding(binding, checked)} />
    ) },
    { key: "status", header: t("extensions.table.status"), render: (binding) => {
      const pending = props.pendingBindings.has(binding.id);
      const feedback = props.bindingFeedback.get(binding.id);
      return <div className="asb-ext-table-cell-stack">
        <span className="asb-pill-status">{t(FILE_STATE_LABELS[binding.fileState])}</span>
        {binding.desired === "disabled" && <span className="asb-pill-status">{t("extensions.bindings.disabled")}</span>}
        {pending && <span className="asb-scope-note" role="status">{t("extensions.bindings.applying")}</span>}
        {!pending && feedback && <span className="asb-warn-text" role="status">
          {"kind" in feedback ? outcomeText(feedback) : commandErrorText(feedback, t)}
        </span>}
        {binding.warnings.map((warning) => <span key={warning} className="asb-warn-text">{warning}</span>)}
      </div>;
    } },
  ];
  if (props.item.kind === "skill") columns.push({
    key: "version", header: t("extensions.table.version"), render: (binding) => binding.lockedDigest
      ? <span className="asb-pill-status">{t("extensions.bindings.pinned", { digest: binding.lockedDigest.slice(0, 12) })}</span>
      : <span className="asb-scope-note">{t("extensions.bindings.followCurrent")}</span>,
  });
  columns.push({ key: "actions", header: t("extensions.table.actions"),
    render: (binding) => <BindingActions binding={binding} props={props} /> });
  const needsDeploy = props.item.bindings.some((binding) => binding.desired === "enabled" && binding.fileState === "pendingApply");
  return <SettingsGroup title={t("extensions.bindings.title")} surface="plain" actions={needsDeploy && (
    <Button variant="secondary" disabled={props.busy} onClick={props.onDeployCurrent}>
      <PreviewIcon />{t("extensions.management.deployCurrent")}
    </Button>
  )}>
    {props.item.bindings.length === 0 ? (
      <div className="asb-empty-state">
        <span className="asb-empty-state-icon" aria-hidden="true"><ConnectivityIcon /></span>
        <h3 className="asb-section-title">{t("extensions.bindings.empty")}</h3>
      </div>
    ) : <Table columns={columns} rows={props.item.bindings} rowKey={(binding) => binding.id}
      ariaLabel={t("extensions.bindings.title")} className="asb-ext-management-table" />}
  </SettingsGroup>;
}

export function ExtensionInstallTargets({
  item, projects, projectNames, busy, installTargets, onInstallTargetsChange, onInstall,
}: ExtensionManagementProps) {
  const { t } = useI18n();
  const existing = new Set(item.bindings.map((binding) => targetValue(binding.target)));
  const options = extensionTargetOptions(item, projects).filter((option) => !existing.has(option.value));
  if (options.length === 0) return null;
  const selected = options.filter((option) => installTargets.includes(option.value)).map((option) => option.value);
  return <SettingsGroup title={t("extensions.install.title")} surface="plain" actions={
    <Button variant="primary" disabled={busy || selected.length === 0} onClick={() => onInstall(selected)}>
      {t("extensions.install.selected")}
    </Button>
  }>
      <Table rows={options} rowKey={(option) => option.value} ariaLabel={t("extensions.install.listAria")}
        className="asb-ext-management-table" columns={[
          { key: "client", header: t("extensions.table.client"), render: (option) => (
            <Checkbox checked={selected.includes(option.value)} label={clientName(option.target.client)}
              ariaLabel={`${t("extensions.install.listAria")} ${option.label}`} disabled={busy}
              onChange={(checked) => onInstallTargetsChange(checked
                ? [...selected, option.value] : selected.filter((value) => value !== option.value))} />
          ) },
          { key: "scope", header: t("extensions.table.scope"), render: (option) => t(option.target.scope === "app"
            ? "extensions.target.app" : option.target.scope === "projectShared"
              ? "extensions.target.projectShared" : "extensions.target.projectPrivate") },
          { key: "project", header: t("extensions.table.project"), render: (option) => option.target.scope === "app"
            ? "—" : projectNames.get(option.target.projectId) ?? option.target.projectId },
        ]} />
  </SettingsGroup>;
}
