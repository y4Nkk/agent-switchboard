import { useEffect, useMemo, useState } from "react";
import type { AppKind, RequestUsageQuery, RequestUsageRange } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { useRequestUsage } from "../../pages/use-request-usage";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { Time } from "../Time";
import { ModuleHeader } from "../WorkspaceHeader";
import { RequestUsageGroups } from "./RequestUsageGroups";
import { RequestUsageRows } from "./RequestUsageRows";
import { RequestUsageScope } from "./RequestUsageScope";
import { RequestUsageSummary } from "./RequestUsageSummary";
import { RequestUsageTrends } from "./RequestUsageTrends";

export interface RequestUsageProvider { app: AppKind; id: string; name: string }
const RANGES: RequestUsageRange[] = ["last15Minutes", "lastHour", "last24Hours", "last7Days", "last30Days", "all"];
const INITIAL_QUERY: RequestUsageQuery = { range: "last24Hours", app: null, profileId: null, model: null, page: 1, groupPage: 1 };

export function RequestUsagePanel({ active, providers, refreshToken }: {
  active: boolean; providers: RequestUsageProvider[]; refreshToken: number;
}) {
  const { t } = useI18n();
  const [query, setQuery] = useState(INITIAL_QUERY);
  const [model, setModel] = useState("");
  const usage = useRequestUsage(active, query, refreshToken);
  const { report, loading, error } = usage;
  const partial = report?.sources.some((source) => source.error !== null) ?? false;
  const names = useMemo(() => new Map(providers.map((provider) => [`${provider.app}/${provider.id}`, provider.name])), [providers]);
  const providerName = (app: AppKind, id: string) => names.get(`${app}/${id}`) ?? t("usagePerformance.unknownProvider", { id });
  const selectedProvider = query.profileId && query.app ? { value: JSON.stringify([query.app, query.profileId]),
    label: providerName(query.app, query.profileId) } : null;
  const update = (patch: Partial<RequestUsageQuery>) => setQuery((current) => ({ ...current, ...patch, page: 1, groupPage: 1 }));
  useEffect(() => {
    const timer = setTimeout(() => setQuery((current) => current.model === (model.trim() || null) ? current
      : { ...current, model: model.trim() || null, page: 1, groupPage: 1 }), 350);
    return () => clearTimeout(timer);
  }, [model]);
  if (!active) return null;
  return <section className="asb-panel asb-request-usage" aria-label={t("usagePerformance.title")}>
    <ModuleHeader title={t("usagePerformance.title")} />
    <div className="asb-request-usage-controls">
      <Select value={query.range} ariaLabel={t("usagePerformance.range")}
        options={RANGES.map((range) => ({ value: range, label: t(`usagePerformance.range.${range}`) }))}
        onChange={(range) => update({ range: range as RequestUsageRange })} />
      <Select value={query.app ?? "all"} ariaLabel={t("usagePerformance.client")}
        options={[{ value: "all", label: t("usagePerformance.allClients") },
          ...(["codex", "claude"] as const).map((app) => ({ value: app, label: clientName(app) }))]}
        onChange={(app) => update({ app: app === "all" ? null : app as AppKind, profileId: null })} />
      <Select value={query.profileId ? JSON.stringify([query.app, query.profileId]) : "all"} ariaLabel={t("usagePerformance.provider")}
        options={[{ value: "all", label: t("usagePerformance.allProviders") },
          ...providers.filter((provider) => query.app === null || query.app === provider.app)
            .map((provider) => ({ value: JSON.stringify([provider.app, provider.id]), label: `${clientName(provider.app)} · ${provider.name}` })),
          ...(selectedProvider && !providers.some((provider) => provider.id === query.profileId && provider.app === query.app)
            ? [selectedProvider] : [])]}
        onChange={(value) => {
          if (value === "all") { update({ profileId: null }); return; }
          const [app, profileId] = JSON.parse(value) as [AppKind, string];
          update({ app, profileId });
        }} />
      <Input value={model} maxLength={512} placeholder={t("usagePerformance.modelFilter")}
        aria-label={t("usagePerformance.modelFilter")} onChange={(event) => setModel(event.target.value)} />
      <Button variant="secondary" disabled={loading} aria-busy={loading} onClick={usage.refresh}>
        {t(loading ? "usage.action.refreshing" : "usage.action.refresh")}</Button>
    </div>
    <p className="asb-scope-note" role="status">{report ? <>
      {t(partial ? "usagePerformance.partialSummary" : error || loading ? "usagePerformance.previous" : "usagePerformance.updated")}
      <Time iso={new Date(report.generatedAtMs).toISOString()} precision="second" /> · {t("usagePerformance.polling")}
    </> : t(loading ? "usagePerformance.loading" : "usagePerformance.noReport")}</p>
    {error && <p className="asb-model-usage-notice" role="alert">
      {t(partial ? "usagePerformance.partialRead" : "usagePerformance.readFailed")} {!partial && error}</p>}
    {report && <>
      <RequestUsageScope report={report} />
      <RequestUsageSummary metrics={report.summary} />
      {report.summary.requests === 0 ? <p className="asb-chart-empty">
        {t(partial ? "usagePerformance.noVerifiedRecords" : "usagePerformance.empty")}</p> : <>
        <RequestUsageTrends report={report} />
        <RequestUsageGroups report={report} providerName={providerName}
          onProvider={(app, profileId) => update({ app, profileId })}
          onPage={(groupPage) => setQuery((current) => ({ ...current, groupPage }))} />
        <RequestUsageRows report={report} providerName={providerName}
          onPage={(page) => setQuery((current) => ({ ...current, page }))} />
      </>}
    </>}
  </section>;
}
