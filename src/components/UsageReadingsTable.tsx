import type { UsageReading } from "../api/client";
import { formatUsageValue } from "../lib/usage-format";
import { UsageRatioMeter } from "./charts/UsageRatioMeter";
import { Table, type TableColumn } from "./Table";

function usageProgress(reading: UsageReading): number | null {
  if (reading.total === null || !Number.isFinite(reading.total) || reading.total <= 0) return null;
  const used = reading.used ?? (reading.remaining === null ? null : reading.total - reading.remaining);
  if (used === null || !Number.isFinite(used)) return null;
  return (used / reading.total) * 100;
}

/** One reading as a table row: plan identity plus its three measures. */
interface UsageRow {
  key: string;
  name: string;
  reading: UsageReading;
}

const USAGE_COLUMNS: Array<TableColumn<UsageRow>> = [
  { key: "plan", header: "额度", render: (row) => row.name },
  {
    key: "remaining",
    header: "余额",
    render: (row) => formatUsageValue(row.reading.remaining, row.reading.unit),
  },
  {
    key: "used",
    header: "已用",
    render: (row) => formatUsageValue(row.reading.used, row.reading.unit),
  },
  {
    key: "total",
    header: "总量",
    render: (row) => formatUsageValue(row.reading.total, row.reading.unit),
  },
  {
    key: "ratio",
    header: "占比",
    render: (row) => {
      const progress = usageProgress(row.reading);
      return (
        <UsageRatioMeter
          percent={progress}
          ariaLabel={`${row.name} 已用比例`}
        />
      );
    },
  },
];

interface Props {
  readings: UsageReading[];
  ariaLabel: string;
}

/** The single usage-readings table: every usage module renders its readings
 * through this owner so the measure columns stay one contract. */
export function UsageReadingsTable({ readings, ariaLabel }: Props) {
  return (
    <Table
      columns={USAGE_COLUMNS}
      rows={readings.map((reading, index) => ({
        key: `${reading.planName ?? "默认"}-${index}`,
        name: reading.planName?.trim() || "默认额度",
        reading,
      }))}
      rowKey={(row) => row.key}
      ariaLabel={ariaLabel}
    />
  );
}
