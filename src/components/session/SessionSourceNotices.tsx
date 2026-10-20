import type { SessionIssue } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientFullName } from "../../lib/client-name";
import { Button } from "../Button";
import { WarningNotice, type WarningNoticeItem } from "../WarningNotice";
import type { SessionConversion } from "./use-session-conversion";

export function SessionSourceNotices({ issues, conversion, disabled }: {
  issues: SessionIssue[];
  conversion: SessionConversion;
  disabled: boolean;
}) {
  const { t } = useI18n();
  const ids = [...new Set(issues.flatMap((issue) => issue.conversionId ? [issue.conversionId] : []))];
  const items: WarningNoticeItem[] = issues.map((issue) => ({
    id: JSON.stringify([issue.app, issue.conversionId, issue.message]),
    content: t("sessions.issues.item", { client: clientFullName(issue.app), message: issue.message }),
  }));
  if (conversion.error) items.push({ id: `conversion-error:${conversion.error}`, content: conversion.error });
  if (conversion.outcome && conversion.outcome.failures.length > 0) items.push({
    id: `conversion-failures:${conversion.outcome.failures.map((failure) => failure.conversionId).join(":")}`,
    content: t("sessions.convert.failures", { count: conversion.outcome.failures.length }),
  });
  for (const failure of conversion.outcome?.failures ?? []) items.push({
    id: `conversion-failure:${failure.app}:${failure.message}`, content: failure.message,
  });
  const unique = [...new Map(items.map((item) => [item.id, item])).values()];
  return <>
    {conversion.outcome && conversion.outcome.converted > 0 && <p className="asb-scope-note" role="status">
      {t("sessions.convert.done", { count: conversion.outcome.converted, records: conversion.outcome.records })}</p>}
    <WarningNotice label={t("sessions.issues.aria")} items={unique} disabled={disabled}
      actions={ids.length > 0 ? <>
        <span className="asb-warning-note">{t("sessions.convert.note")}</span>
        <Button variant="primary" disabled={disabled} onClick={() => void conversion.convert(ids)}>
          {conversion.busy ? t("sessions.convert.busy") : t("sessions.convert.action", { count: ids.length })}</Button>
      </> : undefined} />
  </>;
}
