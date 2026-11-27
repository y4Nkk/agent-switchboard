import { useId } from "react";
import type { RequestUsageQuery, RequestUsageRange } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { RadioOption } from "../RadioOption";
import { SearchInput } from "../SearchInput";
import { Select } from "../Select";

const RANGES: RequestUsageRange[] = ["last15Minutes", "lastHour", "last24Hours", "last7Days", "last30Days", "all"];
export function RequestUsageFilters({ query, model, onModelChange, onChange }: {
  query: RequestUsageQuery;
  model: string;
  onModelChange: (value: string) => void;
  onChange: (patch: Partial<RequestUsageQuery>) => void;
}) {
  const { t } = useI18n();
  const scopeName = useId();
  return <div className="asb-request-usage-controls">
    <div className="asb-segments" role="radiogroup" aria-label={t("usagePerformance.measurementScope")}>
      {(["generation", "transport"] as const).map((scope) => <RadioOption key={scope}
        name={scopeName} checked={query.scope === scope} disabled={false}
        label={t(`usagePerformance.scope.${scope}`)} onChange={() => onChange({ scope })} />)}
    </div>
    <Select value={query.range} ariaLabel={t("usagePerformance.range")}
      options={RANGES.map((range) => ({ value: range, label: t(`usagePerformance.range.${range}`) }))}
      onChange={(range) => onChange({ range: range as RequestUsageRange })} />
    <Select value={query.app ?? "all"} ariaLabel={t("usagePerformance.client")}
      options={[{ value: "all", label: t("usagePerformance.allClients") },
        ...(["codex", "claude"] as const).map((app) => ({ value: app, label: clientName(app) }))]}
      onChange={(app) => onChange({ app: app === "all" ? null : app as "codex" | "claude" })} />
    <SearchInput value={model} maxLength={512} placeholder={t("usagePerformance.modelFilter")}
      ariaLabel={t("usagePerformance.modelFilter")} onChange={onModelChange} />
  </div>;
}
