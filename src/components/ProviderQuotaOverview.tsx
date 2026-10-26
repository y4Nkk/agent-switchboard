import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";
import type { QuotaProvider } from "../lib/quota-profiles";
import { Table, type TableColumn } from "./Table";
import { ModuleHeader } from "./WorkspaceHeader";
import { UsageSummaryView } from "./UsageSummaryView";
import { useProviderUsage } from "./use-provider-usage";

function QuotaReading({ profile }: { profile: QuotaProvider }) {
  const { t } = useI18n();
  const usage = useProviderUsage(profile);
  return <UsageSummaryView name={profile.name} readings={usage.data?.readings ?? []}
    at={usage.data?.at ?? null} querying={usage.querying} error={usage.error}
    empty={t("usage.summary.notQueried")} run={usage.run} showAllMeasures />;
}

export function ProviderQuotaOverview({ profiles }: { profiles: readonly QuotaProvider[] }) {
  const { t } = useI18n();
  const columns: Array<TableColumn<QuotaProvider>> = [
    { key: "client", header: t("usage.quota.client"), render: (profile) => clientName(profile.app) },
    { key: "provider", header: t("usage.quota.provider"), render: (profile) => profile.name },
    { key: "readings", header: t("usage.quota.readings"), render: (profile) => profile.usageQuery
      ? <QuotaReading key={`${profile.app}:${profile.id}:${JSON.stringify(profile.usageQuery)}`} profile={profile} />
      : <span className="asb-scope-note">{t("usage.quota.notConfigured")}</span> },
  ];
  return <section className="asb-panel asb-quota-overview" aria-label={t("usage.quota.providers")}>
    <ModuleHeader title={t("usage.quota.providers")} />
    <p className="asb-scope-note">{t("usage.quota.separateUnits")}</p>
    {profiles.length > 0 ? <Table className="asb-provider-quota-table" columns={columns} rows={[...profiles]} rowKey={(profile) => `${profile.app}:${profile.id}`}
      ariaLabel={t("usage.quota.providers")} /> : <p className="asb-scope-note">{t("usage.quota.noProviders")}</p>}
  </section>;
}
