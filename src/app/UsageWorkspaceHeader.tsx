import { Button } from "../components/Button";
import { Tabs } from "../components/Tabs";
import { Time } from "../components/Time";
import { WorkspaceHeader } from "../components/WorkspaceHeader";
import { useI18n } from "../i18n";
import type { useModelUsageReport } from "../pages/use-model-usage-report";
import { USAGE_SECTIONS, type UsageSection } from "./navigation";

interface Props {
  id: string;
  section: UsageSection;
  onSectionChange: (section: UsageSection) => void;
  usage: ReturnType<typeof useModelUsageReport>;
  consumptionActive: boolean;
  onOpenHistory: () => void;
}

export function UsageWorkspaceHeader({ id, section, onSectionChange, usage, consumptionActive, onOpenHistory }: Props) {
  const { t } = useI18n();
  const consumption = section === "consumption";
  const current = usage.read?.report;
  const previous = usage.loading || usage.error || usage.read?.freshness === "cached";
  const status = current ? <>{t(previous ? "usage.status.snapshot" : "usage.status.current")}<Time iso={current.generatedAt} /></>
    : !usage.loading ? t("usage.status.noSummary") : null;
  return <WorkspaceHeader title={t("nav.page.usage")} primary={
    <Tabs value={section} onChange={onSectionChange} scope={id} label={t("usage.tabs.aria")}
      tabs={USAGE_SECTIONS.map((tab) => ({ value: tab.value, label: t(tab.labelKey), controls: `${id}-${tab.value}-panel` }))} />
  } primaryActions={consumption ? <Button variant="secondary" aria-busy={usage.loading}
    disabled={usage.loading || !consumptionActive} onClick={usage.refresh}>
    {t(usage.loading ? "usage.action.refreshing" : "usage.action.refresh")}
  </Button> : section === "radar" ? <Button variant="secondary" onClick={onOpenHistory}>
    {t("usage.radar.history")}
  </Button> : null} secondary={consumption && status && <p className="asb-header-status" role="status">{status}</p>} />;
}
