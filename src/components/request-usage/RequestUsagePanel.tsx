import { useI18n } from "../../i18n";
import { Button } from "../Button";
import type { useRequestUsage } from "../../pages/use-request-usage";
import { NativeTelemetrySetup } from "./NativeTelemetrySetup";
import { RequestUsageAnalysis } from "./RequestUsageAnalysis";
import { RequestUsageEmpty } from "./RequestUsageEmpty";
import { RequestUsageDetails } from "./RequestUsageDetails";
import { RequestUsageSummary } from "./RequestUsageSummary";

export function RequestUsagePanel({ performance }: { performance: ReturnType<typeof useRequestUsage> }) {
  const { t } = useI18n();
  const { active, report, error, refresh, analysis, setAnalysis, setGroupPage, setPage, showAll } = performance;
  return <div className="asb-panel asb-request-usage">
    <NativeTelemetrySetup active={active} onApplied={refresh} />
    {error && <p className="asb-field-error" role="alert">{t("usagePerformance.readFailed")} {error}</p>}
    {report && report.summary.records > 0 ? <>
      <RequestUsageSummary metrics={report.summary} scope={report.scope} />
      {report.summary.mixedMeasurements && analysis.view !== "models" && <div className="asb-request-usage-notice">
        <span>{t("usagePerformance.mixedMeasurements")}</span>
        <Button variant="ghost" size="compact" onClick={() => setAnalysis({ ...analysis, view: "models" })}>
          {t("usagePerformance.view.models")}
        </Button>
      </div>}
      <RequestUsageAnalysis report={report} selection={analysis} onChange={setAnalysis}
        onGroupPage={setGroupPage} onPage={setPage} />
    </> : !error && <RequestUsageEmpty state={report ? "empty" : "loading"} onShowAll={showAll} />}
    {report && (report.summary.records > 0 || report.sources.some((source) => source.verifiedRecords > 0))
      && <RequestUsageDetails report={report} />}
  </div>;
}
