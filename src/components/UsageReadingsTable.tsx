import type { UsageReading } from "../api/client";
import { useI18n, type TFunction } from "../i18n";
import { formatUsageValue, usageProgress, usageStatusLabel, usageTone } from "../lib/usage-format";
import { UsageRatioMeter } from "./charts/UsageRatioMeter";
import { Table, type TableColumn } from "./Table";
import { Time } from "./Time";

/** One reading as a table row: plan identity plus its three measures. */
interface UsageRow {
  key: string;
  name: string;
  reading: UsageReading;
}

function usageColumns(t: TFunction): Array<TableColumn<UsageRow>> {
  return [
    { key: "plan", header: t("usage.readings.column.plan"), render: (row) => row.name },
    {
      key: "remaining",
      header: t("usage.readings.column.remaining"),
      render: (row) => formatUsageValue(row.reading.remaining, row.reading.unit),
    },
    {
      key: "used",
      header: t("usage.column.used"),
      render: (row) => formatUsageValue(row.reading.used, row.reading.unit),
    },
    {
      key: "total",
      header: t("usage.column.total"),
      render: (row) => formatUsageValue(row.reading.total, row.reading.unit),
    },
    {
      key: "ratio",
      header: t("usage.column.ratio"),
      render: (row) => {
        const progress = row.reading.availability.kind === "unavailable" ? null : usageProgress(row.reading);
        return (
          <UsageRatioMeter
            percent={progress}
            ariaLabel={t("usage.ratioAria", { name: row.name })}
          />
        );
      },
    },
    { key: "reset", header: t("usage.readings.column.reset"), render: ({ reading }) => reading.resetsAt ? <Time iso={reading.resetsAt} /> : "—" },
    { key: "status", header: t("usage.readings.column.status"), render: ({ reading }) => <>
      {usageStatusLabel(reading) && <span className={usageTone(reading) === "danger" ? "asb-usage-invalid" : undefined}>{usageStatusLabel(reading)}</span>}
      {reading.extra && <span className="asb-usage-note">{reading.extra}</span>}
      {reading.availability.kind === "unknown" && !reading.extra && "—"}
    </> },
  ];
}

interface Props {
  readings: UsageReading[];
  ariaLabel: string;
}

/** The single usage-readings table: every usage module renders its readings
 * through this owner so the measure columns stay one contract. */
export function UsageReadingsTable({ readings, ariaLabel }: Props) {
  const { t } = useI18n();
  return (
    <Table
      columns={usageColumns(t)}
      rows={readings.map((reading, index) => ({
        key: `${reading.planName ?? "default"}-${index}`,
        name: reading.planName?.trim() ? reading.planName : t("usage.readings.defaultName"),
        reading,
      }))}
      rowKey={(row) => row.key}
      ariaLabel={ariaLabel}
    />
  );
}
