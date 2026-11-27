import { useId } from "react";
import {
  type CodexOfficialQuotaReset,
} from "../api/client";
import type { MessageKey } from "../i18n";
import { useI18n } from "../i18n";
import { OfficialQuotaTrend } from "./OfficialQuotaTrend";
import { Button } from "./Button";
import { FactList } from "./FactList";
import { Time } from "./Time";
import { QuotaWindowsTable } from "./QuotaWindowsTable";
import { ModuleHeader } from "./WorkspaceHeader";
import type { useCodexOfficialReset } from "./quota-reads";
import { UsageIcon } from "./icons";

const RESET_KIND_LABELS: Record<CodexOfficialQuotaReset["kind"], MessageKey> = {
  early: "codex.officialReset.kindEarly",
  scheduled: "codex.officialReset.kindRoutine",
};

export function CodexOfficialResetPanel({ read }: { read: ReturnType<typeof useCodexOfficialReset> }) {
  const { t } = useI18n();
  const heading = useId();
  const { quota, freshness, cacheLoading, cacheError, readError, statusNotice, history } = read;

  return (
    <section className="asb-usage-surface asb-official-reset" aria-labelledby={heading}>
      <ModuleHeader id={heading} title={t("codex.officialReset.title")} primary={quota?.at &&
        <span className="asb-header-status">{t(freshness === "cached" ? "codex.reset.cachedAt" : "codex.officialReset.readAt")} <Time iso={quota.at} /></span>}
        primaryActions={<Button variant="secondary" size="compact" disabled={read.loading} aria-busy={read.loading}
          onClick={() => void read.readStatus()}>{t(read.loading ? "usage.quota.reading" : "usage.quota.refreshOfficial")}</Button>} />
      {cacheLoading && quota === null && (
        <div className="asb-quota-loading" role="status" aria-label={t("codex.reset.loadingCache")}>
          <span className="asb-skeleton" aria-hidden="true" /><span className="asb-skeleton" aria-hidden="true" />
        </div>
      )}
      {quota === null && !cacheLoading && !cacheError && !statusNotice && !readError && (
        <div className="asb-empty-state">
          <span className="asb-empty-state-icon" aria-hidden="true">
            <UsageIcon />
          </span>
          <h3 className="asb-section-title">{t("codex.officialReset.empty")}</h3>
        </div>
      )}
      {cacheError && <p className="asb-warn-text" role="alert">{t("codex.reset.cacheUnavailable", { error: cacheError })}</p>}
      {statusNotice && <p className="asb-warn-text" role="alert">{statusNotice}</p>}
      {readError && (
        <p className="asb-warn-text" role="alert">
          {t("codex.officialReset.readFailed", { error: readError })}
          {quota !== null ? t("codex.reset.showingStale") : ""}
        </p>
      )}
      {quota !== null && (
        <>
          <QuotaWindowsTable windows={quota.windows} ariaLabel={t("codex.officialReset.windowsAria")} />
          {quota.lastReset ? <FactList inline rows={[
            { label: t("codex.officialReset.detectedReset"), value: <>
              {t(RESET_KIND_LABELS[quota.lastReset.kind])} · <Time iso={quota.lastReset.observedAt} />
            </> },
            ...(quota.lastReset.resetsAt ? [{ label: t("codex.officialReset.updatedResetTime"), value: <Time iso={quota.lastReset.resetsAt} /> }] : []),
          ]} /> : <p className="asb-official-reset-meta">{t("codex.officialReset.noResetDetected")}</p>}
          <OfficialQuotaTrend
            series={history.series}
            loading={history.loading}
            error={history.error}
            ariaLabel={t("codex.officialReset.trendAria")}
          />
        </>
      )}
    </section>
  );
}
