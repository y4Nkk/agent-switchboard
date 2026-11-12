import { useI18n } from "../../i18n";
import { Button } from "../Button";
import type { RequestUsageReport } from "../../api/client";
import type { useRequestUsage } from "../../pages/use-request-usage";
import { NativeTelemetrySetup } from "./NativeTelemetrySetup";
import { RequestUsageAnalysis } from "./RequestUsageAnalysis";
import { RequestUsageBreakdown } from "./RequestUsageBreakdown";
import { RequestUsageEmpty } from "./RequestUsageEmpty";
import { RequestUsageSummary } from "./RequestUsageSummary";
import { useScopeTransition } from "./use-scope-transition";

export function RequestUsagePanel({ performance }: { performance: ReturnType<typeof useRequestUsage> }) {
  const { t } = useI18n();
  const { active, report, error, refresh, analysis, setAnalysis, setGroupPage, setPage, showAll, query } = performance;
  const stage = useScopeTransition(report, query.scope);
  const content = (report: RequestUsageReport | null) => {
    if (!report) return !error && <RequestUsageEmpty state="loading" onShowAll={showAll} />;
    return <>
      {report.summary.records > 0 ? <>
        <RequestUsageSummary metrics={report.summary} scope={report.scope} />
        {report.summary.mixedMeasurements && analysis.view !== "models" && <div className="asb-request-usage-notice">
          <span>{t("usagePerformance.mixedMeasurements")}</span>
          <Button variant="ghost" size="compact" onClick={() => setAnalysis({ ...analysis, view: "models" })}>
            {t("usagePerformance.view.models")}
          </Button>
        </div>}
        <RequestUsageAnalysis report={report} selection={analysis} onChange={setAnalysis}
          onGroupPage={setGroupPage} onPage={setPage} />
      </> : !error && <RequestUsageEmpty state="empty" onShowAll={showAll} />}
      {(report.summary.records > 0 || report.sources.some((source) => source.verifiedRecords > 0))
        && <RequestUsageBreakdown report={report} />}
    </>;
  };
  return <div className="asb-panel asb-request-usage">
    <NativeTelemetrySetup active={active} onApplied={refresh} />
    {error && <p className="asb-field-error" role="alert">{t("usagePerformance.readFailed")} {error}</p>}
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
