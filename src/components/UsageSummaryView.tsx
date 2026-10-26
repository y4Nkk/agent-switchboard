import { RefreshCw } from "lucide-react";
import type { UsageReading } from "../api/client";
import { useI18n } from "../i18n";
import { formatUsageValue, usagePrimary, usageTone, usageStatusLabel } from "../lib/usage-format";
import { Button } from "./Button";
import { Time } from "./Time";
import { Tooltip } from "./Tooltip";

function ReadingSummary({ reading, showAllMeasures }: { reading: UsageReading; showAllMeasures: boolean }) {
  const { t } = useI18n();
  const primary = usagePrimary(reading);
  return (
    <div className="asb-usage-reading" data-tone={usageTone(reading)}>
      {reading.planName && <span className="asb-usage-plan">{reading.planName}</span>}
      {primary && <span className="asb-usage-primary">
        <span>{primary.label}</span><strong>{formatUsageValue(primary.value, reading.unit)}</strong>
      </span>}
      {usageTone(reading) === "danger" && <span className="asb-usage-invalid">{usageStatusLabel(reading)}</span>}
      {(showAllMeasures || reading.unit !== "%") && <span className="asb-usage-secondary">
        {/* The primary chip already shows the used/total measure when it won
            the primary slot; these guards restate that structurally instead of
            comparing display labels. */}
        {reading.used !== null && reading.remaining !== null && <span>{t("usage.reading.usedValue", { value: formatUsageValue(reading.used, reading.unit) })}</span>}
        {reading.total !== null && (reading.remaining !== null || reading.used !== null) && <span>{t("usage.reading.totalValue", { value: formatUsageValue(reading.total, reading.unit) })}</span>}
      </span>}
      {reading.resetsAt && <span className="asb-usage-reset">{t("usage.reading.resetPrefix")}<Time iso={reading.resetsAt} mode="reset" /></span>}
      {reading.extra && <span className="asb-usage-note">{reading.extra}</span>}
    </div>
  );
}

interface Props {
  name: string;
  readings: UsageReading[];
  at: string | null;
  querying: boolean;
  error: string | null;
  stale?: boolean;
  empty: string;
  showAllMeasures?: boolean;
  run: () => Promise<void>;
}

/** Complete decision facts stay visible; detailed tables are a separate disclosure. */
export function UsageSummaryView({ name, readings, at, querying, error, stale, empty, run, showAllMeasures = false }: Props) {
  const { t } = useI18n();
  const hasReadings = readings.length > 0;
  const previous = hasReadings && (stale || Boolean(error));
  return (
    <section className="asb-usage-summary" aria-label={t("usage.summary.nameAria", { name })} aria-busy={querying}>
      <div className="asb-usage-readings">
        {hasReadings ? readings.map((reading, index) => <ReadingSummary key={index} reading={reading} showAllMeasures={showAllMeasures} />)
          : <span role="status">{error ? t("usage.summary.readFailed") : querying ? t("usage.summary.reading") : empty}</span>}
      </div>
      <div className="asb-usage-freshness">
        {previous && <span className="asb-usage-stale">{error ? t("usage.summary.previousFailed") : t("usage.summary.previous")}</span>}
        {at && <span>{t("usage.summary.updatedAt")}<Time iso={at} mode="relative" /></span>}
        <Tooltip label={error ? t("usage.summary.retryAria", { name }) : t("usage.summary.refreshAria", { name })}>
          <Button variant="icon" className="asb-row-action" disabled={querying} aria-label={error ? t("usage.summary.retryAria", { name }) : t("usage.summary.refreshAria", { name })}
            onClick={() => void run()}><RefreshCw size={14} /></Button>
        </Tooltip>
        {querying && <span role="status">{t("usage.summary.updating")}</span>}
      </div>
      {error && <p className="asb-usage-error" role="alert">{error}</p>}
    </section>
  );
}
