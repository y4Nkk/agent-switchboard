import type { CodexOfficialQuotaWindow } from "../api/client";
import { countdownLabel } from "../lib/time";
import { Table, type TableColumn } from "./Table";
import { Time } from "./Time";

function percent(usedPercent: number): string {
  return `${Math.round(usedPercent)} %`;
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
    render: (window) => (
      <div
        className="asb-provider-usage-progress"
        role="progressbar"
        aria-label={`${window.label} 已用比例`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(window.usedPercent)}
      >
        <span style={{ width: `${window.usedPercent}%` }} />
      </div>
    ),
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
