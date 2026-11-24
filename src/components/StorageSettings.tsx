import { useCallback, useEffect, useId, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { archiveHistory, clearSessionIndex, clearWebviewCache, getStorageUsage, previewHistoryArchive,
  type HistoryArchiveOutcome, type HistoryArchivePlan, type StorageUsage } from "../api/storage";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { useI18n } from "../i18n";
import { useMessageState } from "../i18n/use-message-state";
import { localizedMessageText } from "../i18n/errors";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { DatePicker } from "./DatePicker";
import { ModuleHeader } from "./WorkspaceHeader";
import { StorageArtwork } from "./StorageArtwork";
import { Time } from "./Time";

function defaultCutoff() {
  const date = new Date(); date.setUTCDate(date.getUTCDate() - 90);
  return date.toISOString().slice(0, 10);
}

function archiveCount(plan: HistoryArchivePlan) {
  return plan.configuration.codex.records + plan.configuration.claude.records;
}

type StorageOperation = { kind: "cache" | "index" | "archive" } | { kind: "preview"; cutoff: string };
type StorageConfirmation = { kind: "index"; bytes: number } | { kind: "archive"; plan: HistoryArchivePlan };
type StorageOutcome = { kind: "index"; bytes: number } | { kind: "archive"; result: HistoryArchiveOutcome };

function useStorage(active: boolean, onArchived: () => void) {
  const [usage, setUsage] = useState<StorageUsage | null>(null);
  const [confirmation, setConfirmation] = useState<StorageConfirmation | null>(null);
  const [outcome, setOutcome] = useState<StorageOutcome | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  const [readError, setReadError] = useMessageState();
  const [reading, setReading] = useState(false);
  const mounted = useRef(false);
  const generation = useRef(0);
  const locked = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current += 1; }; }, []);
  const refresh = useCallback(async () => {
    if (!mounted.current) return;
    const version = ++generation.current;
    setReading(true);
    try {
      const next = await getStorageUsage();
      if (generation.current === version) { setUsage(next); setReadError(null); }
    } catch (caught) {
      if (generation.current === version) setReadError(caught);
    } finally {
      if (generation.current === version) setReading(false);
    }
  }, [setReadError]);
  useEffect(() => {
    if (active) void refresh();
    else {
      setReading(false);
      setConfirmation((current) => current?.kind === "index" ? null : current);
    }
    return () => { generation.current += 1; };
  }, [active, refresh]);
  const requestIndexCleanup = () => {
    if (!usage?.sessionIndexBytes) return;
    setError(null);
    setConfirmation({ kind: "index", bytes: usage.sessionIndexBytes });
  };
  const run = async (operation: StorageOperation) => {
    if (locked.current) return;
    locked.current = true; setWorking(true); setError(null);
    try {
      if (operation.kind === "cache") await clearWebviewCache();
      if (operation.kind === "index" && confirmation?.kind === "index") {
        const bytes = await clearSessionIndex();
        if (mounted.current) {
          setOutcome({ kind: "index", bytes });
          setConfirmation(null);
        }
      }
      if (operation.kind === "preview") {
        const next = await previewHistoryArchive(operation.cutoff);
        if (mounted.current) setConfirmation({ kind: "archive", plan: next });
      }
      if (operation.kind === "archive" && confirmation?.kind === "archive") {
        const next = await archiveHistory(confirmation.plan);
        if (mounted.current && next) {
          setOutcome({ kind: "archive", result: next });
          setConfirmation(null);
          onArchived();
        }
      }
      await refresh();
    } catch (caught) { if (mounted.current) setError(caught); await refresh(); }
    finally { locked.current = false; if (mounted.current) setWorking(false); }
  };
  return { usage, confirmation, setConfirmation, requestIndexCleanup, outcome, working, error, readError, reading, refresh, run };
}

function Meter({ percent, exceeded = false }: { percent: number | null; exceeded?: boolean }) {
  return <div className="asb-storage-meter" aria-hidden="true" data-unknown={percent === null || undefined}
    data-exceeded={exceeded || undefined}>
    {percent !== null && percent > 0 && <span
      style={{ "--asb-storage-meter-fill": `${Math.min(100, percent)}%` } as CSSProperties} />}
  </div>;
}

type Reading = { size: ReactNode; note?: string; percent: number | null };

function BayFacts({ title, reading, exceeded }: {
  title: string; reading: Reading; exceeded?: boolean;
}) {
  return <>
    <h4 className="asb-storage-bay-title">{title}</h4>
    <span className="asb-storage-reading">
      {reading.size}{reading.note && <span className="asb-storage-note">{reading.note}</span>}
    </span>
    <Meter percent={reading.percent} exceeded={exceeded} />
  </>;
}

type StoragePart = "sessionIndexBytes" | "backupBytes" | "historyBytes";
const MEBIBYTE = 1024 * 1024;

function StorageArchive({ reading, disabled, outcome, onPreview, onCutoffChange }: {
  reading: Reading; disabled: boolean; outcome: HistoryArchiveOutcome | null;
  onPreview: (cutoff: string) => void; onCutoffChange: () => void;
}) {
  const { t } = useI18n();
  const [cutoff, setCutoff] = useState(defaultCutoff);
  return <section className="asb-storage-bay asb-storage-bench">
    <div className="asb-storage-bench-facts">
      <BayFacts title={t("storage.history")} reading={reading} />
    </div>
    <div className="asb-storage-bench-controls">
      <label className="asb-cutoff-field">{t("storage.cutoff")}
        <DatePicker value={cutoff} disabled={disabled} ariaLabel={t("storage.cutoff")}
          onChange={(value) => { setCutoff(value); onCutoffChange(); }} />
      </label>
      <Button variant="secondary" disabled={disabled || !cutoff} onClick={() => onPreview(cutoff)}>
        {t("storage.previewArchive")}
      </Button>
    </div>
    <p className="asb-field-help">{t("storage.archivePolicy")}</p>
    {outcome && <div className="asb-storage-receipt" role="status">
      <p>{t("storage.archived", { count: outcome.removed, path: outcome.path })}</p>
      {outcome.warnings.map((warning, index) => <p className="asb-warn-text" key={index}>{localizedMessageText(warning, t)}</p>)}
    </div>}
  </section>;
}

function StorageCache({ usage, bytes, disabled, onClear }: {
  usage: StorageUsage; bytes: (value: number) => string; disabled: boolean; onClear: () => void;
}) {
  const { t } = useI18n();
  const { cacheBytes, cacheBudgetBytes, cacheClearing } = usage;
  const exceeded = cacheBudgetBytes > 0 && cacheBytes > cacheBudgetBytes;
  return <section className="asb-storage-bay asb-storage-bench asb-storage-cache">
    <div className="asb-storage-bench-facts">
      <BayFacts title={t("storage.cache")} exceeded={exceeded} reading={{
        size: <span className="asb-num">{bytes(cacheBytes)}</span>,
        note: t("storage.cacheBudget", { budget: bytes(cacheBudgetBytes) }),
        percent: cacheBudgetBytes > 0 ? (cacheBytes / cacheBudgetBytes) * 100 : null,
      }} />
    </div>
    <div className="asb-storage-bench-controls">
      <Button variant="secondary" disabled={disabled || cacheClearing} onClick={onClear}>
        {t("storage.clearCache")}
      </Button>
    </div>
    <p className="asb-field-help">{t("storage.cachePolicy")}</p>
  </section>;
}

function StorageReview({ confirmation, disabled, working, error, indexSize, onClose, onConfirm }: {
  confirmation: StorageConfirmation; disabled: boolean; working: boolean; error: string | null;
  indexSize: (bytes: number) => string; onClose: () => void; onConfirm: () => void;
}) {
  const { t } = useI18n();
  if (confirmation.kind === "index") {
    return <ConfirmSheet title={t("storage.clearSessionIndex")} confirmLabel={t("storage.clearSessionIndex")}
      busy={working} confirmDisabled={disabled} destructive onCancel={onClose} onConfirm={onConfirm}>
      <p>{t("storage.sessionIndexReview", { size: indexSize(confirmation.bytes) })}</p>
      {error && <p className="asb-warn-text" role="alert">{error}</p>}
    </ConfirmSheet>;
  }
  const { plan } = confirmation;
  const count = archiveCount(plan);
  return <ConfirmSheet title={t("storage.archiveTitle")} confirmLabel={t("storage.archiveConfirm")}
    busy={working} confirmDisabled={disabled || count === 0} destructive onCancel={onClose} onConfirm={onConfirm}>
    <p>{t("storage.archiveReview.head", { count })}<Time iso={plan.cutoff} mode="date" />{t("storage.archiveReview.tail")}</p>
    <p>{t("storage.archiveProtected")}</p>
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
  </ConfirmSheet>;
}

export function StorageSettings({ active, busy, onArchived }: { active: boolean; busy: boolean; onArchived: () => void }) {
  const { t, language } = useI18n();
  const id = useId();
  const visible = useWorkspaceVisible(active);
  const state = useStorage(active, onArchived);
  const { confirmation, outcome, usage } = state;
  const disabled = busy || state.working;
  const pending = !usage && (state.reading || !state.readError);
  const format = (value: number, unit: number, label: string) =>
    `${(value / unit).toLocaleString(language, { maximumFractionDigits: 1 })} ${label}`;
  const bytes = (value: number) => format(value, MEBIBYTE, "MiB");
  const indexSize = (value: number) => value > 0 && value < MEBIBYTE ? format(value, 1024, "KiB") : bytes(value);
  const size = (value: number | undefined, render = bytes): ReactNode => value === undefined
    ? (pending ? <span className="asb-skeleton asb-storage-placeholder" aria-hidden="true" /> : "—")
    : <span className="asb-num">{render(value)}</span>;
  const reading = (key: StoragePart, render = bytes): Reading => {
    const part = usage?.[key];
    const whole = usage?.dataBytes ?? 0;
    const percent = part !== undefined && whole > 0 && part <= whole ? (part / whole) * 100 : null;
    const rounded = percent === null ? null : Math.round(percent);
    const value = rounded === null ? null : rounded === 0 && part ? "<1%" : `${rounded}%`;
    return { size: size(part, render), percent, note: value ? t("storage.share", { percent: value }) : undefined };
  };

  return <section className="asb-panel asb-storage-settings" aria-labelledby={`${id}-heading`}>
    <ModuleHeader id={`${id}-heading`} title={t("storage.title")} />
    <p className="asb-field-help">{t("storage.description")}</p>
    <div className="asb-storage-hero">
      <div className="asb-storage-overview">
        <div className="asb-storage-artwork"><StorageArtwork active={visible} /></div>
        <div className="asb-storage-total">
          <span id={`${id}-whole`} className="asb-storage-total-label">{t("storage.data")}</span>
          <div className="asb-storage-total-line">
            {size(usage?.dataBytes)}
            <Button variant="secondary" size="compact" disabled={disabled || state.reading}
              onClick={() => void state.refresh()}>{t("storage.refresh")}</Button>
          </div>
          {state.reading && <p className="asb-field-help" role="status">{t("storage.reading")}</p>}
          {state.readError && <p className="asb-warn-text" role="alert">
            {t(usage ? "storage.readFailedStale" : "storage.readFailed", { detail: state.readError })}
          </p>}
        </div>
      </div>
      <ul className="asb-storage-bays" role="list" aria-labelledby={`${id}-whole`}>
        <li className="asb-storage-bay">
          <BayFacts title={t("storage.sessionIndex")} reading={reading("sessionIndexBytes", indexSize)} />
          <Button variant="secondary" size="compact" disabled={disabled || !usage?.sessionIndexBytes}
            onClick={state.requestIndexCleanup}>{t("storage.clearSessionIndex")}</Button>
          {outcome?.kind === "index" && <p className="asb-storage-receipt" role="status">
            {t("storage.sessionIndexCleared", { size: indexSize(outcome.bytes) })}
          </p>}
        </li>
        <li className="asb-storage-bay">
          <BayFacts title={t("storage.backups")} reading={reading("backupBytes")} />
        </li>
      </ul>
    </div>
    <StorageArchive reading={reading("historyBytes")} disabled={disabled}
      outcome={outcome?.kind === "archive" ? outcome.result : null}
      onCutoffChange={() => state.setConfirmation(null)}
      onPreview={(cutoff) => void state.run({ kind: "preview", cutoff })} />
    {usage?.cacheMaintenance && <StorageCache usage={usage} bytes={bytes} disabled={disabled}
      onClear={() => void state.run({ kind: "cache" })} />}
    {state.error && !confirmation && <p className="asb-warn-text" role="alert">{state.error}</p>}
    {active && confirmation && <StorageReview confirmation={confirmation} disabled={disabled} working={state.working}
      error={state.error} indexSize={indexSize} onClose={() => state.setConfirmation(null)}
      onConfirm={() => void state.run({ kind: confirmation.kind })} />}
  </section>;
}
