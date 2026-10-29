import { useCallback, useEffect, useRef, useState } from "react";
import { archiveHistory, clearWebviewCache, getStorageUsage, previewHistoryArchive,
  type HistoryArchiveOutcome, type HistoryArchivePlan, type StorageUsage } from "../api/storage";
import { useI18n } from "../i18n";
import { useMessageState } from "../i18n/use-message-state";
import { localizedMessageText } from "../i18n/errors";
import type { LocalizedMessage } from "../api/switching";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { DatePicker } from "./DatePicker";
import { FactList } from "./FactList";
import { ModuleHeader } from "./WorkspaceHeader";
import { Time } from "./Time";

function defaultCutoff() {
  const date = new Date(); date.setUTCDate(date.getUTCDate() - 90);
  return date.toISOString().slice(0, 10);
}

function archiveCount(plan: HistoryArchivePlan) {
  return plan.configuration.codex.records + plan.configuration.claude.records;
}

type StorageOperation = "cache" | "preview" | "archive";

function useStorage(active: boolean, onArchived: () => void) {
  const [usage, setUsage] = useState<StorageUsage | null>(null);
  const [plan, setPlan] = useState<HistoryArchivePlan | null>(null);
  const [outcome, setOutcome] = useState<HistoryArchiveOutcome | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  const mounted = useRef(false);
  const generation = useRef(0);
  const locked = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current += 1; }; }, []);
  const refresh = useCallback(async () => {
    const version = ++generation.current;
    try { const next = await getStorageUsage(); if (mounted.current && generation.current === version) setUsage(next); }
    catch (caught) { if (mounted.current && generation.current === version) setError(caught); }
  }, [setError]);
  useEffect(() => { if (active) void refresh(); return () => { generation.current += 1; }; }, [active, refresh]);
  const run = async (operation: StorageOperation, cutoff: string) => {
    if (locked.current) return;
    locked.current = true; setWorking(true); setError(null);
    try {
      if (operation === "cache") await clearWebviewCache();
      if (operation === "preview") {
        const next = await previewHistoryArchive(cutoff);
        if (mounted.current) setPlan(next);
      }
      if (operation === "archive" && plan) {
        const next = await archiveHistory(plan);
        if (mounted.current && next) { setOutcome(next); setPlan(null); onArchived(); }
      }
      await refresh();
    } catch (caught) { if (mounted.current) setError(caught); await refresh(); }
    finally { locked.current = false; if (mounted.current) setWorking(false); }
  };
  return { usage, plan, setPlan, outcome, working, error, refresh, run };
}

export function StorageSettings({ active, busy, onArchived }: { active: boolean; busy: boolean; onArchived: () => void }) {
  const { t, language } = useI18n();
  const [cutoff, setCutoff] = useState(defaultCutoff);
  const state = useStorage(active, onArchived);
  const disabled = busy || state.working;
  const bytes = (value: number) => `${(value / 1024 / 1024).toLocaleString(language, { maximumFractionDigits: 1 })} MiB`;
  const warnings = (list: LocalizedMessage[]) =>
    list.map((warning, index) => <p className="asb-warn-text" key={index}>{localizedMessageText(warning, t)}</p>);
  return <section className="asb-panel asb-storage-settings" aria-labelledby="storage-settings-heading">
    <ModuleHeader id="storage-settings-heading" title={t("storage.title")} />
    <p className="asb-field-help">{t("storage.description")}</p>
    {state.usage && <FactList rows={[
      { label: t("storage.data"), value: bytes(state.usage.dataBytes) },
      { label: t("storage.backups"), value: bytes(state.usage.backupBytes) },
      { label: t("storage.history"), value: bytes(state.usage.historyBytes) },
      ...(state.usage.cacheMaintenance ? [{ label: t("storage.cache"), value: bytes(state.usage.cacheBytes) }] : []),
    ]} />}
    <div className="asb-session-actions">
      <Button variant="secondary" disabled={disabled} onClick={() => void state.refresh()}>{t("storage.refresh")}</Button>
      {state.usage?.cacheMaintenance && <Button variant="secondary" disabled={disabled || state.usage.cacheClearing}
        onClick={() => void state.run("cache", cutoff)}>{t("storage.clearCache")}</Button>}
    </div>
    {state.usage?.cacheMaintenance && <p className="asb-field-help">{t("storage.cachePolicy", { budget: bytes(state.usage.cacheBudgetBytes) })}</p>}
    <label className="asb-cutoff-field">{t("storage.cutoff")}
      <DatePicker value={cutoff} disabled={disabled} ariaLabel={t("storage.cutoff")}
        onChange={(value) => { setCutoff(value); state.setPlan(null); }} />
    </label>
    <p className="asb-field-help">{t("storage.archivePolicy")}</p>
    <Button variant="secondary" disabled={disabled || !cutoff} onClick={() => void state.run("preview", cutoff)}>{t("storage.previewArchive")}</Button>
    {state.error && <p className="asb-warn-text" role="alert">{state.error}</p>}
    {state.outcome && <div role="status"><p>{t("storage.archived", { count: state.outcome.removed, path: state.outcome.path })}</p>
      {warnings(state.outcome.warnings)}</div>}
    {state.plan && <ConfirmSheet title={t("storage.archiveTitle")} confirmLabel={t("storage.archiveConfirm")}
      busy={state.working} confirmDisabled={disabled || archiveCount(state.plan) === 0}
      destructive onCancel={() => state.setPlan(null)} onConfirm={() => { void state.run("archive", cutoff); }}>
      <p>{t("storage.archiveReview.head", { count: archiveCount(state.plan) })}<Time iso={state.plan.cutoff} mode="date" />{t("storage.archiveReview.tail")}</p>
      <p>{t("storage.archiveProtected")}</p>
      {state.error && <p className="asb-warn-text" role="alert">{state.error}</p>}
    </ConfirmSheet>}
  </section>;
}
