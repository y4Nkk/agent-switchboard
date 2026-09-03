import type { UsageReading } from "../api/client";
import { Table, type TableColumn } from "./Table";

function usageValue(value: number | null): string {
  if (value === null) return "—";
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 }).format(value);
}

function usageProgress(reading: UsageReading): number | null {
  if (reading.total === null || !Number.isFinite(reading.total) || reading.total <= 0) return null;
  const used = reading.used ?? (reading.remaining === null ? null : reading.total - reading.remaining);
  if (used === null || !Number.isFinite(used)) return null;
  return Math.min(100, Math.max(0, (used / reading.total) * 100));
}

function measure(value: number | null, unit: string | null): string {
  if (value === null) return "—";
  return unit === null ? usageValue(value) : `${usageValue(value)} ${unit}`;
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
    render: (row) => measure(row.reading.remaining, row.reading.unit),
  },
  {
    key: "used",
    header: "已用",
    render: (row) => measure(row.reading.used, row.reading.unit),
  },
  {
    key: "total",
    header: "总量",
    render: (row) => measure(row.reading.total, row.reading.unit),
  },
  {
    key: "ratio",
    header: "占比",
    render: (row) => {
      const progress = usageProgress(row.reading);
      if (progress === null) return "—";
      return (
        <div
          className="asb-provider-usage-progress"
          role="progressbar"
          aria-label={`${row.name} 已用比例`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progress)}
        >
          <span style={{ width: `${progress}%` }} />
        </div>
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
