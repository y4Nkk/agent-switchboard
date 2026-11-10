import { useState } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { Time } from "../Time";

export function RequestUsageScope({ report }: { report: RequestUsageReport }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <CollapsibleSection open={open} onOpenChange={setOpen} quiet
    className="asb-request-usage-scope" label={t("usagePerformance.methodology")}>
    <div className="asb-request-usage-scope-body">
      <p className="asb-scope-note">{t("usagePerformance.sourceNote")}</p>
      <div className="asb-request-usage-source-groups">
        <section className="asb-request-usage-facts">
          <h4 className="asb-group-title">{t("usagePerformance.scopeFacts")}</h4>
          <FactList rows={[
            { label: t("usagePerformance.measurementScope"), value: t(`usagePerformance.scope.${report.scope}`) },
            { label: t("usagePerformance.window"), value: <><Time iso={new Date(report.startAtMs).toISOString()} precision="second" />
              {" — "}<Time iso={new Date(report.endAtMs).toISOString()} precision="second" /></> },
            { label: t("usagePerformance.bucketLabel"), value: t("usagePerformance.bucket", { minutes: report.bucketMs / 60_000 }) },
            { label: t("usagePerformance.refreshFrequency"), value: t("usagePerformance.polling") },
          ]} />
        </section>
        {report.sources.map((source) => <section className="asb-request-usage-facts" key={source.app}>
          <h4 className="asb-group-title">{clientName(source.app)}</h4>
          <FactList rows={[
            { label: t("usagePerformance.recordsCount"), value: source.verifiedRecords },
            { label: t("usagePerformance.observedSpan"), value: source.earliestAtMs === null ? t("usagePerformance.notRecorded") : <>
              <Time iso={new Date(source.earliestAtMs).toISOString()} precision="second" />
              {source.latestAtMs !== null && <> — <Time iso={new Date(source.latestAtMs).toISOString()} precision="second" /></>}</> },
          ]} />
        </section>)}
      </div>
      <ul className="asb-request-usage-methodology">
        {(["speed", "first", "completion", "cache", "tokens", "cost", "comparison", "trends"] as const).map((key) =>
          <li key={key}>{t(`usagePerformance.method.${key}`)}</li>)}
      </ul>
    </div>
  </CollapsibleSection>;
}
