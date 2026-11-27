import { memo, useId } from "react";
import type { ModelUsageRange, ModelUsageReport } from "../api/model-usage";
import { ModelUsageActivity } from "../components/ModelUsageActivity";
import { ModelUsageRefreshStatus } from "../components/ModelUsageRefreshStatus";
import { ModelUsageIssues } from "../components/ModelUsageIssues";
import { ModelUsageTable } from "../components/ModelUsageTable";
import { hasKnownUsageTotal, modelUsageSlices, modelUsageTrend, usageOverview } from "../components/model-usage-display";
import { RadioOption } from "../components/RadioOption";
import { ModelUsageDistributionChart } from "../components/charts/ModelUsageDistributionChart";
import { UsageTrendChart } from "../components/charts/UsageTrendChart";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { useI18n, type MessageKey } from "../i18n";
import { formatTokenCount, TOKEN_UNIT } from "../lib/token-format";
import { formatCompactUsageValue } from "../lib/usage-format";
import type { useModelUsageReport } from "./use-model-usage-report";

const RANGES = [
  { value: "today", label: "usage.range.today" },
  { value: "last7Days", label: "usage.range.last7Days" },
  { value: "last30Days", label: "usage.range.last30Days" },
  { value: "all", label: "usage.range.all" },
] as const satisfies ReadonlyArray<{ value: ModelUsageRange; label: MessageKey }>;

function formatSessionSpan(seconds: number | null, language: string): string {
  if (seconds === null) return "—";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor(seconds / 60) % 60;
  const count = (value: number, unit: string) => new Intl.NumberFormat(language, { style: "unit", unit }).format(value);
  if (hours === 0) return minutes === 0 ? count(seconds, "second") : count(minutes, "minute");
  return minutes === 0 ? count(hours, "hour") : `${count(hours, "hour")} ${count(minutes, "minute")}`;
}

const UsageOverview = memo(function UsageOverview({ report }: { report: ModelUsageReport }) {
  const { t, language } = useI18n();
  const { total, peak, currentStreak, longestStreak } = usageOverview(report);
  const measured = hasKnownUsageTotal(report);
  const hasDatedUsage = measured && (peak > 0 || total === 0);
  const days = new Intl.NumberFormat(language, { style: "unit", unit: "day", unitDisplay: "long" });
  const metrics = [
    { label: t("usage.label.cumulative"), value: measured ? formatCompactUsageValue(total) : "—",
      title: measured ? `${formatTokenCount(total)} ${TOKEN_UNIT}` : undefined },
    { label: t("usage.label.peak"), value: hasDatedUsage ? formatCompactUsageValue(peak) : "—",
      title: hasDatedUsage ? `${formatTokenCount(peak)} ${TOKEN_UNIT}` : undefined },
    { label: t("usage.label.sessionSpan"), value: formatSessionSpan(report.longestSessionSpanSeconds, language),
      title: t("usage.label.sessionSpanHint") },
    { label: t("usage.label.currentStreak"), value: hasDatedUsage ? days.format(currentStreak) : "—" },
    { label: t("usage.label.longestStreak"), value: hasDatedUsage ? days.format(longestStreak) : "—" },
  ];
  return <dl className="asb-consumption-card asb-consumption-summary" aria-label={t("usage.summary.groupAria")}>
    {metrics.map(({ label, value, title }) => <div key={label}>
      <dt>{label}</dt>
      <dd title={title}>{value}</dd>
    </div>)}
  </dl>;
});

function UsageRange({ range, onRangeChange }: { range: ModelUsageRange; onRangeChange: (range: ModelUsageRange) => void }) {
  const { t } = useI18n();
  const group = useId();
  return <div className="asb-consumption-row">
    <h4 id={`${group}-label`}>{t("usage.range.title")}</h4>
    <div className="asb-segments asb-consumption-segments" role="radiogroup" aria-labelledby={`${group}-label`}>
      {RANGES.map((option) => <RadioOption key={option.value} name={group} checked={range === option.value}
        disabled={false} label={t(option.label)} onChange={() => onRangeChange(option.value)} />)}
    </div>
  </div>;
}

const UsageDetails = memo(function UsageDetails({ report }: { report: ModelUsageReport }) {
  const { t } = useI18n();
  const slices = modelUsageSlices(report, t);
  if (slices.length === 0) return <>
    <p className="asb-consumption-card asb-consumption-empty" role="status">{t("usage.empty")}</p>
    <ModelUsageIssues issues={report.issues} />
  </>;
  return <>
    <UsageTrendChart series={modelUsageTrend(report, slices, t)} ariaLabel={t("usage.trend.chartAria")}
      title={t("usage.trend.title")} emptyMessage={t("usage.trend.empty")} valueKind="local-token" />
    <ModelUsageDistributionChart items={slices} ariaLabel={t("usage.composition.chartAria")} />
    <ModelUsageTable report={report} />
    <ModelUsageIssues issues={report.issues} />
  </>;
});

export function UsagePage({ usage, range, onRangeChange }: {
  usage: ReturnType<typeof useModelUsageReport>;
  range: ModelUsageRange;
  onRangeChange: (range: ModelUsageRange) => void;
}) {
  const { t } = useI18n();
  const heading = useId();
  const { read, loading, error } = usage;
  return <section className="asb-consumption" aria-labelledby={heading}>
    <ModuleHeader title={t("usage.consumption")} id={heading} />
    {error && <p className="asb-consumption-error" role="alert">
      {t(loading ? "usage.refresh.retrying" : read ? "usage.refresh.failedSaved" : "usage.refresh.failed")} {error}
    </p>}
    <ModelUsageRefreshStatus activity={usage.activity} task={usage.task} hasReport={read !== null} />
    {read && <>
      <UsageOverview report={read.lifetime} />
      <ModelUsageActivity report={read.lifetime} />
      <UsageRange range={range} onRangeChange={onRangeChange} />
      <UsageDetails report={read.report} />
    </>}
  </section>;
}
