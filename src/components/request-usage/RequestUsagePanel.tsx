import { useEffect, useMemo, useState } from "react";
import type { AppKind, RequestUsageQuery } from "../../api/client";
import { useI18n } from "../../i18n";
import { useRequestUsage } from "../../pages/use-request-usage";
import { Button } from "../Button";
import { Time } from "../Time";
import { ModuleHeader } from "../WorkspaceHeader";
import { RequestUsageAnalysis, type RequestUsageAnalysisSelection } from "./RequestUsageAnalysis";
import { RequestUsageEmpty } from "./RequestUsageEmpty";
import { RequestUsageDetails } from "./RequestUsageDetails";
import { RequestUsageFilters, type RequestUsageProvider } from "./RequestUsageFilters";
import { RequestUsageScope } from "./RequestUsageScope";
import { RequestUsageSummary } from "./RequestUsageSummary";

const INITIAL_QUERY: RequestUsageQuery = { range: "last24Hours", app: null, profileId: null, model: null, page: 1, groupPage: 1 };

export function RequestUsagePanel({ active, providers, refreshToken }: {
  active: boolean; providers: RequestUsageProvider[]; refreshToken: number;
}) {
  const { t } = useI18n();
  const [query, setQuery] = useState(INITIAL_QUERY);
  const [model, setModel] = useState("");
  const [analysis, setAnalysis] = useState<RequestUsageAnalysisSelection>({ view: "trend", metric: "speedP50" });
  const usage = useRequestUsage(active, query, refreshToken);
  const { report, loading, error } = usage;
  const partial = report?.sources.some((source) => source.error !== null) ?? false;
  const emptyState = partial ? "partial" : error ? "unavailable" : report ? "empty" : "loading";
  const filtered = query.range !== "all" || query.app !== null || query.profileId !== null || query.model !== null || model.trim() !== "";
  const names = useMemo(() => new Map(providers.map((provider) => [`${provider.app}/${provider.id}`, provider.name])), [providers]);
  const providerName = (app: AppKind, id: string) => names.get(`${app}/${id}`) ?? t("usagePerformance.unknownProvider", { id });
  const update = (patch: Partial<RequestUsageQuery>) => setQuery((current) => ({ ...current, ...patch, page: 1, groupPage: 1 }));
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
          <span>{t(partial ? "usagePerformance.partialSummary" : error || loading ? "usagePerformance.previous" : "usagePerformance.updated")}
            <Time iso={new Date(report.generatedAtMs).toISOString()} mode="relative" /></span>
          <span>{t("usagePerformance.verifiedCount", { count: report.summary.requests })}</span>
        </> : <span>{t(loading ? "usagePerformance.loading" : "usagePerformance.noReport")}</span>}
      </p>
    } primaryActions={<Button variant="secondary" disabled={loading} aria-busy={loading} onClick={usage.refresh}>
      {t(loading ? "usage.action.refreshing" : "usage.action.refresh")}</Button>}
      secondary={<RequestUsageFilters query={query} providers={providers} providerName={providerName}
        model={model} onModelChange={setModel} onChange={update} />} />
    {error && <div className="asb-model-usage-notice asb-request-usage-error" role="alert">
      <p>{t(partial ? "usagePerformance.partialRead" : "usagePerformance.readFailed")}</p><p>{error}</p>
    </div>}
    {report && report.summary.requests > 0 ? <>
      <RequestUsageSummary metrics={report.summary} />
      <RequestUsageAnalysis report={report} providerName={providerName} selection={analysis} onChange={setAnalysis}
        onProvider={(app, profileId) => update({ app, profileId })}
        onGroupPage={(groupPage) => setQuery((current) => ({ ...current, groupPage }))}
        onPage={(page) => setQuery((current) => ({ ...current, page }))} />
      <RequestUsageDetails metrics={report.summary} />
    </> : <RequestUsageEmpty state={emptyState} onShowAll={filtered ? () => {
      setModel(""); setQuery({ ...INITIAL_QUERY, range: "all" });
    } : undefined} />}
    {report && <RequestUsageScope report={report} />}
  </section>;
}
