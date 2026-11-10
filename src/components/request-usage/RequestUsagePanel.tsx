import { useEffect, useState } from "react";
import type { RequestUsageQuery } from "../../api/client";
import { useI18n } from "../../i18n";
import { useRequestUsage } from "../../pages/use-request-usage";
import { Button } from "../Button";
import { Time } from "../Time";
import { ModuleHeader } from "../WorkspaceHeader";
import { NativeTelemetrySetup } from "./NativeTelemetrySetup";
import { RequestUsageAnalysis, type RequestUsageAnalysisSelection } from "./RequestUsageAnalysis";
import { RequestUsageEmpty } from "./RequestUsageEmpty";
import { RequestUsageDetails } from "./RequestUsageDetails";
import { RequestUsageFilters } from "./RequestUsageFilters";
import { RequestUsageScope } from "./RequestUsageScope";
import { RequestUsageSummary } from "./RequestUsageSummary";

const INITIAL_QUERY: RequestUsageQuery = { range: "last24Hours", app: null, scope: "generation", model: null, page: 1, groupPage: 1 };

export function RequestUsagePanel({ active, refreshToken }: { active: boolean; refreshToken: number }) {
  const { t } = useI18n();
  const [query, setQuery] = useState(INITIAL_QUERY);
  const [model, setModel] = useState("");
  const [analysis, setAnalysis] = useState<RequestUsageAnalysisSelection>({ view: "trend", metric: "speedP50" });
  const usage = useRequestUsage(active, query, refreshToken);
  const { report, loading, error } = usage;
  const filtered = query.range !== "all" || query.app !== null || query.model !== null || model.trim() !== "";
  const update = (patch: Partial<RequestUsageQuery>) => {
    if (patch.scope && patch.scope !== query.scope) setAnalysis((current) => ({ ...current,
      metric: patch.scope === "transport" ? "durationP50Ms" : "speedP50" }));
    setQuery((current) => ({ ...current, ...patch, page: 1, groupPage: 1 }));
  };
  useEffect(() => {
    const timer = setTimeout(() => setQuery((current) => current.model === (model.trim() || null) ? current
      : { ...current, model: model.trim() || null, page: 1, groupPage: 1 }), 350);
    return () => clearTimeout(timer);
  }, [model]);
  if (!active) return null;
  return <section className="asb-panel asb-request-usage" aria-label={t("usagePerformance.title")}>
    <ModuleHeader title={t("usagePerformance.title")} primary={
      <p className="asb-request-usage-status" role="status">
        <span>{t("usagePerformance.sourceShort")}</span>
        {report ? <>
          <span>{t(error || loading ? "usagePerformance.previous" : "usagePerformance.updated")}
            <Time iso={new Date(report.generatedAtMs).toISOString()} mode="relative" /></span>
          <span>{t("usagePerformance.verifiedCount", { count: report.summary.records })}</span>
        </> : <span>{t(loading ? "usagePerformance.loading" : "usagePerformance.noReport")}</span>}
      </p>
    } primaryActions={<Button variant="secondary" disabled={loading} aria-busy={loading} onClick={usage.refresh}>
      {t(loading ? "usage.action.refreshing" : "usage.action.refresh")}</Button>}
      secondary={<RequestUsageFilters query={query} model={model} onModelChange={setModel} onChange={update} />} />
    <NativeTelemetrySetup active={active} onApplied={usage.refresh} />
    <p className="asb-scope-note">{t(`usagePerformance.scopeNote.${query.scope}`)}</p>
    {error && <div className="asb-model-usage-notice asb-request-usage-error" role="alert">
      <p>{t("usagePerformance.readFailed")}</p><p>{error}</p>
    </div>}
    {report && report.summary.records > 0 ? <>
      <RequestUsageSummary metrics={report.summary} scope={report.scope} />
      <RequestUsageAnalysis report={report} selection={analysis} onChange={setAnalysis}
        onGroupPage={(groupPage) => setQuery((current) => ({ ...current, groupPage }))}
        onPage={(page) => setQuery((current) => ({ ...current, page }))} />
      <RequestUsageDetails metrics={report.summary} />
    </> : <RequestUsageEmpty state={error ? "unavailable" : report ? "empty" : "loading"}
      onShowAll={filtered ? () => { setModel(""); setQuery({ ...INITIAL_QUERY, scope: query.scope, range: "all" }); } : undefined} />}
    {report && <RequestUsageScope report={report} />}
  </section>;
}
