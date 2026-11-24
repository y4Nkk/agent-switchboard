import { useId } from "react";
import type { ProviderConnectionOptions, UpstreamProtocol, UsageQuery } from "../api/client";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { WorkspaceSubpage } from "./WorkspaceSubpage";
import { Tabs } from "./Tabs";
import { Time } from "./Time";
import { UsageIcon } from "./icons";
import { useUnsavedChanges } from "./useUnsavedChanges";
import { UsageReadingsTable } from "./UsageReadingsTable";
import { DeclarativeFields, RefreshInterval, ScriptFields } from "./usage-query/UsageQueryFields";
import { canRun, useUsageQueryDraft, useUsageQueryExecution } from "./usage-query/useUsageQueryEditor";

export interface UsageQueryWorkspaceProps {
  active: boolean;
  providerName: string;
  value: UsageQuery | null;
  apiKey: string;
  authentication?: import("../api/shared").AuthenticationScheme | null;
  connection?: ProviderConnectionOptions | null;
  baseUrl: string | null;
  upstreamProtocol: UpstreamProtocol | null;
  busy: boolean;
  onSave: (next: UsageQuery | null) => Promise<boolean> | boolean;
  onClose: () => void;
}

export function UsageQueryWorkspace(props: UsageQueryWorkspaceProps) {
  const { t } = useI18n();
  const execution = useUsageQueryExecution(props);
  const editor = useUsageQueryDraft(props.value, execution.clearResult);
  const { draft, kind } = editor;
  const disabled = props.busy || execution.querying || execution.saving;
  const leave = useUnsavedChanges(editor.dirty, disabled, { guardPageRoot: props.active });
  const scope = useId();
  return <WorkspaceSubpage layout="fixed-actions" className="asb-usage-editor" title={t("usage.query.title")} backLabel={t("usage.query.back")}
    busy={disabled} onBack={() => leave.request(props.onClose)} primary={<>
      <p className="asb-usage-provider">{props.providerName.trim() || t("usage.query.unnamedProvider")}</p>
      <Tabs value={kind} onChange={editor.selectKind} scope={scope} label={t("usage.query.modeAria")}
        tabs={[{ value: "declarative", label: t("usage.query.modeDeclarative"), disabled, controls: `${scope}-declarative-panel` },
          { value: "script", label: t("usage.query.modeScript"), disabled, controls: `${scope}-script-panel` }]} /></>}
    footer={<Button variant="primary" className="asb-editor-submit" disabled={disabled}
      onClick={() => void execution.save(draft)}>{execution.saving ? t("usage.query.saving") : t("usage.query.save")}</Button>}>
    <section className="asb-usage-workspace" aria-label={t("usage.query.title")} onKeyDown={(event) => {
      if (event.key === "Escape" && !event.defaultPrevented) leave.request(props.onClose);
    }}>
      <div className="asb-editor-section" role="tabpanel" id={`${scope}-declarative-panel`}
        aria-labelledby={`${scope}-declarative-tab`} hidden={kind !== "declarative"}>
        <DeclarativeFields value={editor.declarative} disabled={disabled} onChange={editor.patchDeclarative} /></div>
      <div className="asb-editor-section" role="tabpanel" id={`${scope}-script-panel`}
        aria-labelledby={`${scope}-script-tab`} hidden={kind !== "script"}>
        <ScriptFields source={draft?.kind === "script" ? draft.source : ""} disabled={disabled} onChange={editor.patchScript} /></div>
      <RefreshInterval text={editor.intervalText} disabled={disabled} onChange={editor.setIntervalText} onCommit={editor.commitInterval} />
      <section className="asb-editor-section" aria-label={t("usage.query.testTitle")}>
        <h3 className="asb-section-title">{t("usage.query.testTitle")}</h3><div className="asb-editor-section-fields">
          <div className="asb-editor-action-row"><Button variant="secondary" className="asb-usage-run" disabled={disabled || !canRun(draft)}
            onClick={() => void execution.run(draft)}><UsageIcon />{execution.querying ? t("usage.query.running") : t("usage.query.run")}</Button>
            <span className="asb-field-help">{execution.querying ? t("usage.query.querying") : canRun(draft) ? t("usage.query.ready") : t("usage.query.needConfig")}</span></div>
          {execution.error && <p className="asb-warn-text" role="alert">{execution.error}</p>}
        </div></section>
      {execution.summary && <section className="asb-editor-section" aria-label={t("usage.query.resultAria")}>
        <h3 className="asb-section-title">{t("usage.query.resultTitle")}</h3><div className="asb-editor-section-fields">
          <div className="asb-usage-readout-head"><Time iso={execution.summary.at} /></div>
          <UsageReadingsTable readings={execution.summary.readings} ariaLabel={t("usage.query.readingsAria")} />
        </div></section>}
    </section>{leave.confirmation}
  </WorkspaceSubpage>;
}
