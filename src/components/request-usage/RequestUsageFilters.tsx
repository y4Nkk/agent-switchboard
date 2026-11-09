import type { AppKind, RequestUsageQuery, RequestUsageRange } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Input } from "../Input";
import { Select } from "../Select";
import type { ProviderName } from "./RequestUsageGroups";

export interface RequestUsageProvider { app: AppKind; id: string; name: string }
const RANGES: RequestUsageRange[] = ["last15Minutes", "lastHour", "last24Hours", "last7Days", "last30Days", "all"];

export function RequestUsageFilters({ query, providers, providerName, model, onModelChange, onChange }: {
  query: RequestUsageQuery;
  providers: RequestUsageProvider[];
  providerName: ProviderName;
  model: string;
  onModelChange: (value: string) => void;
  onChange: (patch: Partial<RequestUsageQuery>) => void;
}) {
  const { t } = useI18n();
  const selectedProvider = query.profileId && query.app ? { value: JSON.stringify([query.app, query.profileId]),
    label: providerName(query.app, query.profileId) } : null;
  return <div className="asb-request-usage-controls">
    <Select value={query.range} ariaLabel={t("usagePerformance.range")}
      options={RANGES.map((range) => ({ value: range, label: t(`usagePerformance.range.${range}`) }))}
      onChange={(range) => onChange({ range: range as RequestUsageRange })} />
    <Select value={query.app ?? "all"} ariaLabel={t("usagePerformance.client")}
      options={[{ value: "all", label: t("usagePerformance.allClients") },
        ...(["codex", "claude"] as const).map((app) => ({ value: app, label: clientName(app) }))]}
      onChange={(app) => onChange({ app: app === "all" ? null : app as AppKind, profileId: null })} />
    <Select value={query.profileId ? JSON.stringify([query.app, query.profileId]) : "all"}
      ariaLabel={t("usagePerformance.provider")} options={[
        { value: "all", label: t("usagePerformance.allProviders") },
        ...providers.filter((provider) => query.app === null || query.app === provider.app)
          .map((provider) => ({ value: JSON.stringify([provider.app, provider.id]), label: `${clientName(provider.app)} · ${provider.name}` })),
        ...(selectedProvider && !providers.some((provider) => provider.id === query.profileId && provider.app === query.app)
          ? [selectedProvider] : []),
      ]} onChange={(value) => {
        if (value === "all") { onChange({ profileId: null }); return; }
        const [app, profileId] = JSON.parse(value) as [AppKind, string];
        onChange({ app, profileId });
      }} />
    <Input value={model} maxLength={512} placeholder={t("usagePerformance.modelFilter")}
      aria-label={t("usagePerformance.modelFilter")} onChange={(event) => onModelChange(event.target.value)} />
  </div>;
}
