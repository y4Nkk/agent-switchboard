import { useId } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { Tabs } from "../Tabs";
import { RequestUsageGroups } from "./RequestUsageGroups";
import { RequestUsageRows } from "./RequestUsageRows";
import { RequestUsageTrends, type RequestUsageTrend } from "./RequestUsageTrends";

export interface RequestUsageAnalysisSelection { view: "trend" | "models" | "requests"; metric: RequestUsageTrend }
export function RequestUsageAnalysis({ report, selection, onChange, onGroupPage, onPage }: {
  report: RequestUsageReport;
  selection: RequestUsageAnalysisSelection;
  onChange: (selection: RequestUsageAnalysisSelection) => void;
  onGroupPage: (page: number) => void;
  onPage: (page: number) => void;
}) {
  const { t } = useI18n();
  const id = useId();
  const views = ["trend", "models", "requests"] as const;
  return <div className="asb-request-usage-analysis">
    <Tabs value={selection.view} scope={id} label={t("usagePerformance.analysis")}
      onChange={(view) => onChange({ ...selection, view })} tabs={views.map((view) => ({
        value: view, label: t(`usagePerformance.view.${view}`), controls: `${id}-${view}-panel`,
      }))} />
    {views.map((view) => <div key={view} id={`${id}-${view}-panel`} role="tabpanel"
      aria-labelledby={`${id}-${view}-tab`} hidden={selection.view !== view}>
      {selection.view === view && (view === "trend"
        ? <RequestUsageTrends report={report} metric={selection.metric}
          onMetricChange={(metric) => onChange({ ...selection, metric })} />
        : view === "models" ? <RequestUsageGroups report={report} onPage={onGroupPage} />
          : <RequestUsageRows report={report} onPage={onPage} />)}
    </div>)}
  </div>;
}
