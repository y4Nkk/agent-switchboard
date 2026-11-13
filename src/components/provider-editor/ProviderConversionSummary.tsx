import type { ProviderConversion, UsageQuery } from "../../api/client";
import { selectedConversionUsage } from "../../lib/provider-conversion";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { AppDialog } from "../AppDialog";
import { Checkbox } from "../Checkbox";

export function ProviderConversionBlocked({ conversion, onClose }: {
  conversion: ProviderConversion; onClose: () => void;
}) {
  const { t } = useI18n();
  return <AppDialog title={t("providers.conversion.unavailable")} busy={false} narrow onClose={onClose}>
    <p>{conversion.source.name}</p>
    {conversion.blockedReason && <p>{localizedMessageText(conversion.blockedReason, t)}</p>}
  </AppDialog>;
}

export function ProviderConversionSummary({ conversion, busy, usageQuery, onUsageChange }: {
  conversion: ProviderConversion; busy: boolean; usageQuery: UsageQuery | null;
  onUsageChange: (query: UsageQuery | null) => void;
}) {
  const { t } = useI18n();
  const target = conversion.targetApp === "codex" ? "Codex" : "Claude";
  const source = conversion.source.app === "codex" ? "Codex" : "Claude";
  return <section className="asb-editor-section" aria-label={t("providers.conversion.summary")}>
    <h3 className="asb-section-title">{t("providers.conversion.from", { source, name: conversion.source.name, target })}</h3>
    <div className="asb-editor-section-fields">
      <p className="asb-field-help">{t("providers.conversion.snapshot")}</p>
      {conversion.notices.length > 0 && <ul>{conversion.notices.map((notice, index) =>
        <li key={`${notice.key}-${index}`}>{localizedMessageText(notice, t)}</li>)}</ul>}
      {conversion.usageQuery && <Checkbox label={t("providers.conversion.copyUsage")} checked={usageQuery !== null}
        disabled={busy} onChange={(checked) => onUsageChange(selectedConversionUsage(conversion.usageQuery, checked))} />}
    </div>
  </section>;
}
