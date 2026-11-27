import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Time } from "../Time";
import type { RequestUsageReport } from "../../api/client";
import type { useRequestUsage } from "../../pages/use-request-usage";
import { NativeTelemetrySetup } from "./NativeTelemetrySetup";
import { RequestUsageAnalysis } from "./RequestUsageAnalysis";
import { RequestUsageBreakdown } from "./RequestUsageBreakdown";
import { RequestUsageEmpty } from "./RequestUsageEmpty";
import { RequestUsageFilters } from "./RequestUsageFilters";
import { RequestUsageSummary } from "./RequestUsageSummary";
import { useScopeTransition } from "./use-scope-transition";

export function RequestUsagePanel({ performance }: { performance: ReturnType<typeof useRequestUsage> }) {
  const { t } = useI18n();
  const { active, report, error, loading, refresh, analysis, setAnalysis, setGroupPage, setPage, showAll,
    query, model, setModel, updateFilters } = performance;
  const stage = useScopeTransition(report, query.scope);
  const content = (report: RequestUsageReport | null) => {
    if (!report) return !error && <RequestUsageEmpty state="loading" onShowAll={showAll} />;
    return <>
      {report.summary.records > 0 ? <>
        <RequestUsageSummary report={report} />
        <RequestUsageAnalysis report={report} selection={analysis} onChange={setAnalysis}
          onGroupPage={setGroupPage} onPage={setPage} />
      </> : !error && <RequestUsageEmpty state="empty" onShowAll={showAll} />}
      {(report.summary.records > 0 || report.sources.some((source) => source.verifiedRecords > 0))
        && <RequestUsageBreakdown report={report} />}
    </>;
  };
  return <div className="asb-request-usage asb-usage-stack">
    <NativeTelemetrySetup active={active} onApplied={refresh} />
    <div className="asb-request-usage-query">
      <div className="asb-request-usage-toolbar">
        <RequestUsageFilters query={query} model={model} onModelChange={setModel} onChange={updateFilters} />
        <Button variant="secondary" aria-busy={loading} disabled={loading || !active} onClick={refresh}>
          {t(loading ? "usage.action.refreshing" : "usage.action.refresh")}
        </Button>
      </div>
      {report && <p className="asb-request-usage-status" role="status">
        <span>{t(error || loading ? "usagePerformance.previous" : "usagePerformance.updated")}
          <Time iso={new Date(report.generatedAtMs).toISOString()} mode="relative" /></span>
      </p>}
      {error && <p className="asb-field-error" role="alert">{t("usagePerformance.readFailed")} {error}</p>}
    </div>
    <div className="asb-request-usage-stage" ref={stage.stageRef}>
      {stage.exit && <div className="asb-request-usage-stage-layer" data-scope-leaving inert aria-hidden="true"
        ref={stage.bindExitLayer}>
        {content(stage.exit.report)}
      </div>}
      <div className="asb-request-usage-stage-layer" ref={stage.bindCurrentLayer}>
        {content(report)}
      </div>
    </div>
  </div>;
}
