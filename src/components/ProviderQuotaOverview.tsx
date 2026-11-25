import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";
import type { QuotaProvider } from "../lib/quota-profiles";
import { Table, type TableColumn } from "./Table";
import { ModuleHeader } from "./WorkspaceHeader";
import { UsageSummaryView } from "./UsageSummaryView";
import { quotaKey, type useProviderQuotaRead } from "../pages/use-provider-quota-read";

export function ProviderQuotaOverview({ profiles, quota }: {
  profiles: readonly QuotaProvider[];
  quota: ReturnType<typeof useProviderQuotaRead>;
}) {
  const { t } = useI18n();
  const columns: Array<TableColumn<QuotaProvider>> = [
    { key: "client", header: t("usage.quota.client"), render: (profile) => clientName(profile.app) },
    { key: "provider", header: t("usage.quota.provider"), render: (profile) => profile.name },
    { key: "readings", header: t("usage.quota.readings"), render: (profile) => {
      if (!profile.usageQuery) return <span className="asb-scope-note">{t("usage.quota.notConfigured")}</span>;
      const row = quota.row(profile);
      return <UsageSummaryView name={profile.name} readings={row.summary?.readings ?? []}
        at={row.summary?.at ?? null} querying={row.querying} error={row.error}
        empty={t(quota.ready ? "usage.summary.notQueried" : "usage.summary.reading")}
        run={row.run} showAllMeasures />;
    } },
  ];
  return <section className="asb-panel asb-quota-overview" aria-label={t("usage.quota.providers")}>
    <ModuleHeader title={t("usage.quota.providers")} />
    <p className="asb-scope-note">{t("usage.quota.separateUnits")}</p>
    {profiles.length > 0 ? <Table className="asb-provider-quota-table" columns={columns} rows={[...profiles]} rowKey={quotaKey}
      ariaLabel={t("usage.quota.providers")} /> : <p className="asb-scope-note">{t("usage.quota.noProviders")}</p>}
  </section>;
}
