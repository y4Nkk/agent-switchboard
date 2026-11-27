import { useState } from "react";
import type { ExtensionManagementProps } from "./management-types";
import type { McpCheckResult } from "../../api/client";
import { useI18n } from "../../i18n";
import { tr } from "../../i18n/current";
import { Button } from "../Button";
import { Select } from "../Select";
import { SettingsGroup, SettingsRow } from "../SettingsLayout";
import { Time } from "../Time";
import { Table } from "../Table";
import { extensionTargetOptions } from "./targets";
import { parseTargetValue, targetLabel, targetValue } from "./labels";

function outcomeView(result: McpCheckResult): { badge: string; tone: string; text: string } {
  const outcome = result.outcome;
  switch (outcome.kind) {
    case "passed":
      return {
        badge: tr("extensions.check.passed"),
        tone: "asb-ok-text",
        text: tr("extensions.check.passedDetail", {
          protocol: result.protocolVersion ?? tr("extensions.check.unknown"),
          tools: outcome.tools,
          resources: outcome.resources,
          prompts: outcome.prompts,
        }),
      };
    case "partial":
      return { badge: tr("extensions.check.partial"), tone: "asb-warn-text", text: outcome.error };
    case "failed":
      return {
        badge: tr("extensions.check.failed"),
        tone: "asb-fail-text",
        text: tr("extensions.check.failedDetail", { classification: outcome.classification, message: outcome.error }),
      };
    case "cancelled":
      return { badge: tr("extensions.check.cancelled"), tone: "", text: "" };
    case "needsNativeConfirmation":
      return {
        badge: tr("extensions.check.needsNativeConfirmation"),
        tone: "asb-warn-text",
        text: tr("extensions.check.nativeConfirmationDetail"),
      };
  }
}
function CheckResults({ item, projectNames, check }: ExtensionManagementProps) {
  const { t } = useI18n();
  if (item.checks.length === 0) return null;
  const rows = item.checks.map((result) => ({ result, view: outcomeView(result) }));
  /* The live region predates the table's own scroll wrap: it exists so the
     results table's replacement is announced, and carries no layout of its
     own. */
  return <div aria-live="polite" aria-busy={check.phase !== null}>
    <Table rows={rows} rowKey={({ result }) => targetValue(result.target)}
      ariaLabel={t("extensions.check.results")} className="asb-ext-management-table" columns={[
        { key: "target", header: t("extensions.table.target"), render: ({ result }) => targetLabel(result.target, projectNames) },
        { key: "result", header: t("extensions.table.result"), render: ({ result, view }) => (
          <div className="asb-ext-table-cell-stack">
            <span className="asb-pill-status">{view.badge}</span>
            {view.text && <span className={view.tone}>{view.text}{result.truncated ? t("extensions.check.truncated") : ""}</span>}
          </div>
        ) },
        { key: "time", header: t("extensions.table.checkedAt"), render: ({ result }) => <Time iso={result.checkedAt} /> },
        { key: "duration", align: "end", header: t("extensions.table.duration"), render: ({ result }) => t("extensions.check.duration", { duration: result.durationMs }) },
      ]} />
  </div>;
}

export function ExtensionCheckPanel(props: ExtensionManagementProps) {
  const { t } = useI18n();
  const { item, projects, busy, check } = props;
  const options = extensionTargetOptions(item, projects);
  const preferred = item.bindings.find((binding) => binding.desired === "enabled") ?? item.bindings[0];
  const [chosen, setChosen] = useState<string | null>(() => preferred ? targetValue(preferred.target) : null);
  const value = options.find((option) => option.value === chosen)?.value ?? options[0]?.value ?? null;
  const target = value ? parseTargetValue(value) : null;
  const ownRun = check.subject?.definitionId === item.id
    && targetValue(check.subject.target) === value;
  return (
    <SettingsGroup title={t("extensions.check.title")} actions={
      <div className="asb-ext-actions">
        <Button variant="secondary" disabled={busy || !target || check.phase !== null}
          onClick={() => { if (target) void check.start({ definitionId: item.id, definitionRevision: item.revision, target }); }}>
          {t("extensions.check.run")}
        </Button>
        {ownRun && check.phase !== null && (
          <Button variant="secondary" onClick={() => void check.cancel()}>{t("confirm.cancel")}</Button>
        )}
      </div>
    }>
      <SettingsRow label={t("extensions.check.target")} description={t("extensions.check.contextHint")}>
        <Select value={value} options={options} onChange={setChosen}
          ariaLabel={t("extensions.check.target")} disabled={busy || check.phase !== null} />
      </SettingsRow>
      {ownRun && check.phase !== null && (
        <p className="asb-scope-note" role="status" aria-live="polite">
          {t(check.phase === "cancelling" ? "extensions.check.cancelling" : check.phase === "starting" ? "extensions.check.starting" : "extensions.check.running")}
        </p>
      )}
      {ownRun && check.failure && <p className="asb-warn-text" role="alert">{check.failure}</p>}
      <CheckResults {...props} />
    </SettingsGroup>
  );
}
