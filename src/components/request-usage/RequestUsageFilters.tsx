import type { MeasurementScope, RequestUsageQuery, RequestUsageRange } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Input } from "../Input";
import { Select } from "../Select";

const RANGES: RequestUsageRange[] = ["last15Minutes", "lastHour", "last24Hours", "last7Days", "last30Days", "all"];
export function RequestUsageFilters({ query, model, onModelChange, onChange }: {
  query: RequestUsageQuery;
  model: string;
  onModelChange: (value: string) => void;
  onChange: (patch: Partial<RequestUsageQuery>) => void;
}) {
  const { t } = useI18n();
  return <div className="asb-request-usage-controls">
    <Select value={query.range} ariaLabel={t("usagePerformance.range")}
      options={RANGES.map((range) => ({ value: range, label: t(`usagePerformance.range.${range}`) }))}
      onChange={(range) => onChange({ range: range as RequestUsageRange })} />
    <Select value={query.app ?? "all"} ariaLabel={t("usagePerformance.client")}
      options={[{ value: "all", label: t("usagePerformance.allClients") },
        ...(["codex", "claude"] as const).map((app) => ({ value: app, label: clientName(app) }))]}
      onChange={(app) => onChange({ app: app === "all" ? null : app as "codex" | "claude" })} />
    <Select value={query.scope} ariaLabel={t("usagePerformance.measurementScope")}
      options={(["generation", "transport"] as const).map((scope) => ({ value: scope, label: t(`usagePerformance.scope.${scope}`) }))}
      onChange={(scope) => onChange({ scope: scope as MeasurementScope })} />
    <Input value={model} maxLength={512} placeholder={t("usagePerformance.modelFilter")}
      aria-label={t("usagePerformance.modelFilter")} onChange={(event) => onModelChange(event.target.value)} />
  </div>;
}
