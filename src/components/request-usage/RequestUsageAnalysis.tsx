import { useId } from "react";
import type { AppKind, RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { Tabs } from "../Tabs";
import { RequestUsageGroups, type ProviderName } from "./RequestUsageGroups";
import { RequestUsageRows } from "./RequestUsageRows";
import { RequestUsageTrends, type RequestUsageTrend } from "./RequestUsageTrends";

export interface RequestUsageAnalysisSelection {
  view: "trend" | "providers" | "requests";
  metric: RequestUsageTrend;
}

export function RequestUsageAnalysis({ report, providerName, selection, onChange, onProvider, onGroupPage, onPage }: {
  report: RequestUsageReport;
  providerName: ProviderName;
  selection: RequestUsageAnalysisSelection;
  onChange: (selection: RequestUsageAnalysisSelection) => void;
  onProvider: (app: AppKind, id: string) => void;
  onGroupPage: (page: number) => void;
  onPage: (page: number) => void;
}) {
  const { t } = useI18n();
  const id = useId();
  const views = ["trend", "providers", "requests"] as const;
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
        : view === "providers"
          ? <RequestUsageGroups report={report} providerName={providerName} onProvider={onProvider} onPage={onGroupPage} />
          : <RequestUsageRows report={report} providerName={providerName} onPage={onPage} />)}
    </div>)}
  </div>;
}
