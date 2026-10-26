import type { CodexOfficialQuotaWindow } from "../api/client";
import { officialQuotaReadings } from "../lib/usage-format";
import { UsageReadingsTable } from "./UsageReadingsTable";

export function QuotaWindowsTable({ windows, ariaLabel }: { windows: CodexOfficialQuotaWindow[]; ariaLabel: string }) {
  return <UsageReadingsTable readings={officialQuotaReadings({ windows })} ariaLabel={ariaLabel} />;
}
