import { useState } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { CollapsibleSection } from "../Collapsible";
import { Time } from "../Time";

export function RequestUsageScope({ report }: { report: RequestUsageReport }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <div className="asb-request-usage-scope">
    <p className="asb-scope-note">{t("usagePerformance.sourceNote")}</p>
    <p className="asb-scope-note">{t("usagePerformance.window")}
      <Time iso={new Date(report.startAtMs).toISOString()} precision="second" />
      {" — "}<Time iso={new Date(report.endAtMs).toISOString()} precision="second" />
      {" · "}{t("usagePerformance.bucket", { minutes: report.bucketMs / 60_000 })}
    </p>
    {report.sources.map((source) => <div key={source.app}>
      <p className="asb-scope-note">{clientName(source.app)} · {t(source.error ? "usagePerformance.sourceVerified"
        : "usagePerformance.rangeVerified", { count: source.verifiedRequests })}
        {source.earliestAtMs !== null && <> · <Time iso={new Date(source.earliestAtMs).toISOString()} precision="second" />
          {source.latestAtMs !== null && <> — <Time iso={new Date(source.latestAtMs).toISOString()} precision="second" /></>}</>}
        {source.retentionLimit !== null && <> · {t("usagePerformance.retentionLimit", { count: source.retentionLimit })}</>}
      </p>
      {source.error && <p className="asb-model-usage-notice" role="alert">
        {t("usagePerformance.sourceError", { client: clientName(source.app) })} {source.error}</p>}
    </div>)}
    {report.summary.speedSamples > 0 && report.summary.speedSamples < 20 &&
      <p className="asb-scope-note">{t("usagePerformance.smallSample")}</p>}
    <CollapsibleSection open={open} onOpenChange={setOpen} quiet label={t("usagePerformance.methodology")}>
      <ul className="asb-request-usage-methodology">
        {(["speed", "first", "completion", "cache", "tokens", "cost", "comparison", "trends"] as const).map((key) =>
          <li key={key}>{t(`usagePerformance.method.${key}`)}</li>)}
      </ul>
    </CollapsibleSection>
  </div>;
}
