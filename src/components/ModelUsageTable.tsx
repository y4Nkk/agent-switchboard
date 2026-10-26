import type { ModelUsageGroup, ModelUsageReport } from "../api/client";
import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";
import { formatTokenCount } from "../lib/token-format";
import { Table, type TableColumn } from "./Table";

export function ModelUsageTable({ report }: { report: ModelUsageReport }) {
  const { t } = useI18n();
  const columns: Array<TableColumn<ModelUsageGroup>> = [
    { key: "app", header: t("usage.column.client"), render: (row) => clientName(row.app) },
    { key: "model", header: t("usage.column.model"), cellClassName: "asb-code",
      render: (row) => row.model ?? t("usage.model.unrecorded") },
    { key: "input", header: t("usage.column.input"), render: (row) => formatTokenCount(row.inputTokens) },
    { key: "cache", header: t("usage.column.cache"),
      render: (row) => formatTokenCount(row.cacheReadInputTokens + row.cacheCreationInputTokens) },
    { key: "output", header: t("usage.column.output"), render: (row) => formatTokenCount(row.outputTokens) },
    { key: "total", header: t("usage.column.totalTokens"), render: (row) => formatTokenCount(row.totalTokens) },
    { key: "sessions", header: t("usage.column.sessions"), render: (row) => formatTokenCount(row.sessionCount) },
  ];
  return <Table columns={columns} rows={report.groups} rowKey={(row) => JSON.stringify([row.app, row.model])}
    ariaLabel={t("usage.consumption")} className="asb-model-usage-table" />;
}
