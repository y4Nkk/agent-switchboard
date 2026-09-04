import type { CodexOfficialQuotaWindow } from "../api/client";
import { formatUsageValue } from "../lib/usage-format";
import { UsageRatioMeter } from "./charts/UsageRatioMeter";
import { countdownLabel } from "../lib/time";
import { Table, type TableColumn } from "./Table";
import { Time } from "./Time";

function percent(usedPercent: number): string {
  return formatUsageValue(usedPercent, "%");
}

const WINDOW_COLUMNS: Array<TableColumn<CodexOfficialQuotaWindow>> = [
  { key: "label", header: "窗口", render: (window) => window.label },
  { key: "used", header: "已用", render: (window) => percent(window.usedPercent) },
  {
    key: "reset",
    header: "重置",
    render: (window) =>
      window.resetsAt === null ? (
        "—"
      ) : (
        <>
          <Time iso={window.resetsAt} /> · {countdownLabel(window.resetsAt)}
        </>
      ),
  },
  {
    key: "ratio",
    header: "占比",
    render: (window) => <UsageRatioMeter percent={window.usedPercent} ariaLabel={`${window.label} 已用比例`} />,
  },
];

interface Props {
  windows: CodexOfficialQuotaWindow[];
  ariaLabel: string;
}

/** The single official-quota windows table: the provider-card quota panel and
 * the settings reset panel render their server windows through this owner. */
export function QuotaWindowsTable({ windows, ariaLabel }: Props) {
  return (
    <Table
      columns={WINDOW_COLUMNS}
      rows={windows}
      rowKey={(window, index) => `${window.label}-${index}`}
      ariaLabel={ariaLabel}
    />
  );
}
