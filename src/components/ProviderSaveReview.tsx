import type { ProviderSaveReview as Review } from "../app/provider-save-review";
import { useI18n } from "../i18n";
import { FactList } from "./FactList";
import { Table } from "./Table";

export function ProviderSaveReview({ review }: { review: Review }) {
  const { t } = useI18n();
  const target = (value: Review["changes"][number]["after"]) => value.target
    ? `${value.target.provider ?? t("modelRoutes.unresolved")} / ${value.target.model || t("modelRoutes.clientDefault")}${value.oneM ? " [1M]" : ""} · ${t(value.mode === "provider" ? "modelRoutes.specify" : "modelRoutes.followPrimary")}`
    : t("modelRoutes.automatic");
  return <>
    <FactList rows={[
      { label: t("modelRoutes.editing"), value: review.editedName },
      { label: t("modelRoutes.activeUnchanged"), value: review.activeName ?? t("modelRoutes.unmatched") },
    ]} />
    {review.changes.length > 0 && <Table rows={review.changes} rowKey={(row) => row.role} ariaLabel={t("modelRoutes.routeChanges")} columns={[
      { key: "role", header: t("modelRoutes.purpose"), render: (row) => t(`modelRoutes.role.${row.role}`) },
      { key: "before", header: t("providers.diff.column.before"), render: (row) => target(row.before) },
      { key: "after", header: t("providers.diff.column.after"), render: (row) => target(row.after) },
    ]} />}
    <p className="asb-field-help">{t("modelRoutes.liveEffect")}</p>
  </>;
}
