import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Pagination } from "../Pagination";
import { GROUP_METRICS, type RequestUsageGroupMetric } from "./chart-metrics";
import { RequestUsagePointAxis, RequestUsagePoints } from "./RequestUsagePoints";

export function RequestUsageGroups({ report, metric, onPage }: {
  report: RequestUsageReport; metric: RequestUsageGroupMetric; onPage: (page: number) => void;
}) {
  const { t, language } = useI18n();
  const definition = GROUP_METRICS[metric];
  const series = report.groups.map((group) => {
    const raw = group.metrics[metric];
    return {
      group,
      value: typeof raw === "string" ? Number(raw) : raw,
      marker: definition.companion ? group.metrics[definition.companion.key] : null,
      samples: definition.samples?.(group.metrics) ?? null,
    };
  });
  const maximum = definition.maximum ?? Math.max(0, ...series.flatMap(({ value, marker }) => [value ?? 0, marker ?? 0]));
  return <div className="asb-request-usage-section">
    <figure className="asb-request-usage-plot" aria-label={`${t("usagePerformance.groups")} · ${t(definition.labelKey)}`}>
      <RequestUsagePointAxis end={maximum > 0 ? definition.format(maximum, language) : null} />
      <ul className="asb-request-usage-observations">
        {series.map(({ group, value, marker, samples }) => <li className="asb-request-usage-observation"
          key={JSON.stringify([group.app, group.model, group.eventName])} data-client={group.app}>
          <span className="asb-request-usage-observation-label">
            <span className="asb-code">{group.model ?? t("usage.model.unrecorded")}</span>
            <span className="asb-request-usage-caption">{clientName(group.app)} · {group.eventName}</span>
          </span>
          <RequestUsagePoints value={value} maximum={maximum} marker={marker} />
          <span className="asb-request-usage-observation-value">
            <strong>{definition.format(value, language)}</strong>
            {definition.companion && marker !== null && <span className="asb-request-usage-caption">
              {t(definition.companion.labelKey)} {definition.format(marker, language)}
            </span>}
            {samples !== null && <span className="asb-request-usage-caption">{t("usagePerformance.samples", { count: samples })}</span>}
          </span>
        </li>)}
      </ul>
    </figure>
    <Pagination total={report.totalGroups} page={report.groupPage} pageSize={report.groupPageSize}
      onPageChange={onPage} label={t("usagePerformance.groupPages")} />
  </div>;
}
