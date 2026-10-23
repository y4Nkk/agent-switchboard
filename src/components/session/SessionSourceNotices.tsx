import { useMemo } from "react";
import type { SessionIssue } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientFullName } from "../../lib/client-name";
import { Button } from "../Button";
import { useInformationSource, type InformationNotice } from "../InformationSources";
import type { SessionConversion } from "./use-session-conversion";

export function SessionSourceNotices({ issues, conversion, disabled }: {
  issues: SessionIssue[];
  conversion: SessionConversion;
  disabled: boolean;
}) {
  const { t } = useI18n();
  const { error, outcome, busy, attempt, convert } = conversion;
  const messages = useMemo(() => {
    const ids = [...new Set(issues.flatMap((issue) => issue.conversionId ? [issue.conversionId] : []))];
    const items: InformationNotice[] = issues.map((issue) => ({
      id: JSON.stringify([issue.app, issue.conversionId, issue.message]), kind: "warning",
      title: t("sessions.issues.item", { client: clientFullName(issue.app), message: issue.message }),
    }));
    if (ids.length) items.push({ id: "session-conversion", kind: "warning",
      title: t("sessions.convert.action", { count: ids.length }), description: t("sessions.convert.note"),
      action: <Button variant="primary" disabled={disabled} onClick={() => void convert(ids)}>
        {busy ? t("sessions.convert.busy") : t("sessions.convert.action", { count: ids.length })}</Button>,
    });
    if (error) items.push({ id: `conversion-error:${attempt}`, kind: "error", title: error });
    if (outcome?.converted) items.push({ id: `conversion-done:${attempt}`, kind: "success",
      title: t("sessions.convert.done", { count: outcome.converted, records: outcome.records }) });
    if (outcome?.failures.length) items.push({ id: `conversion-failures:${attempt}`, kind: "error",
      title: t("sessions.convert.failures", { count: outcome.failures.length }) });
    for (const failure of outcome?.failures ?? []) items.push({
      id: `conversion-failure:${attempt}:${failure.app}:${failure.conversionId}:${failure.message}`, kind: "error", title: failure.message,
    });
    return [...new Map(items.map((item) => [item.id, item])).values()];
  }, [issues, error, outcome, busy, attempt, convert, disabled, t]);
  useInformationSource(messages);
  return null;
}
