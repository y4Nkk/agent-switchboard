import type { ResourceDetails, ResourceProcess } from "../../api/client";
import { useI18n } from "../../i18n";
import { Pagination } from "../Pagination";
import { Table, type TableColumn } from "../Table";
import { ResourceMetric } from "./ResourceMetric";

interface Props {
  details: ResourceDetails | null;
  requestedPage: number | null;
  loading: boolean;
  onPageChange: (page: number) => void;
}

export function ResourceProcessDetails({ details, requestedPage, loading, onPageChange }: Props) {
  const { t } = useI18n();
  if (!details) return <p className="asb-field-help" role="status">
    {t(loading ? "resources.details.loading" : "resources.details.unavailable")}
  </p>;
  const columns: TableColumn<ResourceProcess>[] = [
    { key: "pid", header: t("resources.column.pid"), render: (row) => row.pid },
    { key: "name", header: t("resources.column.process"), render: (row) => <span className="asb-code">{row.name}</span> },
    { key: "role", header: t("resources.column.role"), render: (row) => t(`resources.role.${row.role}`) },
    { key: "evidence", header: t("resources.column.evidence"), render: (row) => t(`resources.evidence.${row.evidence}`) },
    { key: "cpu", header: "CPU", render: (row) => <ResourceMetric metric={row.cpu} kind="cpu" /> },
    { key: "memory", header: t("resources.column.memory"),
      render: (row) => <ResourceMetric metric={row.residentBytes} kind="memory" /> },
  ];
  return <div className="asb-resource-details" aria-busy={loading}>
    {details.rows.length ? <Table columns={columns} rows={details.rows} rowKey={(row) => row.key}
      ariaLabel={t("resources.details.aria")} className="asb-resource-process-table" />
      : <p className="asb-field-help">{t("resources.details.empty")}</p>}
    <p className="asb-resource-page-status asb-field-help" role="status">
      {requestedPage !== null && requestedPage !== details.page
        ? t(loading ? "resources.details.readingPage" : "resources.details.previousPage", {
          requested: requestedPage, current: details.page,
        }) : null}
    </p>
    <Pagination total={details.total} page={details.page} pageSize={details.pageSize}
      onPageChange={onPageChange} label={t("resources.details.pages")} />
  </div>;
}
