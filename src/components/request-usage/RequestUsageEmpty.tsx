import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { UsageIcon } from "../icons";

const COPY = {
  loading: "usagePerformance.loadingTitle",
  empty: "usagePerformance.emptyTitle",
  partial: "usagePerformance.partialTitle",
  unavailable: "usagePerformance.unavailableTitle",
} as const;

export function RequestUsageEmpty({ state, onShowAll }: { state: keyof typeof COPY; onShowAll?: () => void }) {
  const { t } = useI18n();
  return <div className="asb-empty-state asb-request-usage-empty" role="status" aria-busy={state === "loading"}>
    <span className="asb-empty-state-icon" aria-hidden="true"><UsageIcon /></span>
    <h4 className="asb-group-title">{t(COPY[state])}</h4>
    {state === "empty" && onShowAll && <Button variant="secondary" size="compact" onClick={onShowAll}>
      {t("usagePerformance.showAll")}</Button>}
  </div>;
}
