import { useEffect, useRef, useState, type RefObject } from "react";
import type { AppKind } from "../../api/client";
import type { SessionRangeAction, SessionRangePreview, SessionRangeProgress } from "../../api/session-range";
import { useI18n } from "../../i18n";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { DatePicker } from "../DatePicker";
import { Pagination } from "../Pagination";
import { Select } from "../Select";
import { Time } from "../Time";
import { sessionTitle } from "./session-content";
import { useSessionRange, type SessionRangeState } from "./use-session-range";

function RangeForm({ state, app, archived, scopeRef }: {
  state: SessionRangeState; app: AppKind | null; archived: boolean; scopeRef: RefObject<HTMLParagraphElement | null>;
}) {
  const { t } = useI18n();
  const locked = state.busy || state.preview !== null;
  return <>
    <p ref={scopeRef} tabIndex={-1} className="asb-scope-note">{t("sessions.range.scope", { client: app === "codex" ? "Codex" : app === "claude" ? "Claude" : t("clientConfig.filter.all"),
      state: t(archived ? "sessions.archive.title" : "sessions.list.heading") })}</p>
    <div className="asb-session-range-fields">
      <label>{t("sessions.time.from")}<DatePicker value={state.dates.from} disabled={locked}
        ariaLabel={t("sessions.time.from")} onChange={(from) => state.setDates({ ...state.dates, from })} /></label>
      <label>{t("sessions.time.to")}<DatePicker value={state.dates.to} disabled={locked}
        ariaLabel={t("sessions.time.to")} onChange={(to) => state.setDates({ ...state.dates, to })} /></label>
      <label>{t("sessions.range.action")}<Select value={state.action} disabled={locked} ariaLabel={t("sessions.range.action")}
        onChange={(value) => state.setAction(value as SessionRangeAction)} options={[
          { value: archived ? "unarchive" : "archive", label: t(archived ? "sessions.range.unarchive" : "sessions.range.archive") },
          { value: "delete", label: t("sessions.delete.title") },
        ]} /></label>
    </div>
    {!state.valid && <p className="asb-field-error" role="alert">{t("sessions.range.invalidDates")}</p>}
    <p className="asb-field-help">{t("sessions.range.datePolicy")}</p>
    <p className="asb-field-help">{t(state.action === "delete" ? "sessions.range.deletePolicy" : "sessions.range.archivePolicy")}</p>
  </>;
}

function RangePreview({ preview }: { preview: SessionRangePreview }) {
  const { t } = useI18n();
  const [page, setPage] = useState(1);
  const related = preview.items.filter((item) => item.related).length;
  return <section className="asb-session-range-preview" aria-label={t("sessions.range.preview")}>
    <p role="status">{t("sessions.range.summary", { matched: preview.matched, total: preview.items.length })}</p>
    {related > 0 && <p className="asb-warn-text">{t("sessions.range.related", { count: related })}</p>}
    {preview.items.length === 0 && <p>{t("sessions.range.empty")}</p>}
    <ul className="asb-session-range-items">{preview.items.slice((page - 1) * 25, page * 25).map(({ session, related }) =>
      <li key={`${session.app}:${session.sessionId}`}>
        <div><strong>{sessionTitle(session)}</strong><span>{session.app === "codex" ? "Codex" : "Claude"}
          {related && ` · ${t("sessions.range.relatedLabel")}`}</span></div>
        {session.lastActiveAt ? <Time iso={session.lastActiveAt} /> : <span>{t("sessions.time.unknown")}</span>}
      </li>)}</ul>
    <Pagination total={preview.items.length} page={page} pageSize={25} label={t("sessions.range.previewPages")} onPageChange={setPage} />
    {preview.issues.map((issue, index) => <p className="asb-warn-text" key={index}>{issue.message}</p>)}
  </section>;
}

function RangeResults({ progress, action }: { progress: SessionRangeProgress; action: SessionRangeAction }) {
  const { t } = useI18n();
  const [page, setPage] = useState(1);
  const failed = progress.outcomes.filter((item) => !item.completed || item.error);
  const completed = progress.outcomes.filter((item) => item.completed).length;
  return <section aria-label={t("sessions.range.results")}>
    <p role="status">{t(progress.running ? "sessions.range.progress" : "sessions.range.done", {
      completed: progress.running ? progress.completed : completed, total: progress.total, failed: failed.length,
    })}</p>
    {progress.running && <progress className="asb-session-range-progress" value={progress.completed} max={Math.max(1, progress.total)}
      aria-label={t("sessions.range.executing")} />}
    {progress.warning && <p className="asb-warn-text" role="alert">{progress.warning}</p>}
    <ul className="asb-session-range-items">{progress.outcomes.slice((page - 1) * 25, page * 25).map((item) =>
      <li key={`${item.app}:${item.sessionId}`}><div><code>{item.sessionId}</code>
        {item.error ? <span className="asb-warn-text">{item.error}</span> : <span>{t(item.completed
          ? action === "delete" ? "sessions.range.deleted" : action === "archive" ? "sessions.range.archived" : "sessions.range.restored"
          : "sessions.range.kept")}</span>}</div></li>)}</ul>
    <Pagination total={progress.outcomes.length} page={page} pageSize={25} label={t("sessions.range.resultPages")} onPageChange={setPage} />
  </section>;
}

export function SessionRangeDialog({ app, archived, dates, onClose, onChanged }: {
  app: AppKind | null; archived: boolean; dates: { from: string; to: string }; onClose: () => void; onChanged: () => void;
}) {
  const { t } = useI18n();
  const state = useSessionRange(app, archived, dates, onChanged, onClose);
  const scope = useRef<HTMLParagraphElement>(null);
  const wasBusy = useRef(false);
  useEffect(() => {
    if (wasBusy.current && !state.busy) scope.current?.focus();
    wasBusy.current = state.busy;
  }, [state.busy]);
  return <AppDialog title={t("sessions.range.title")} busy={state.busy} onClose={() => void state.close()} wide footer={<>
    {state.preview && <Button variant="secondary" disabled={state.busy} onClick={() => void state.edit()}>{t("sessions.range.change")}</Button>}
    {state.preview ? <Button variant={state.action === "delete" ? "danger" : "primary"}
      disabled={state.busy || !state.confirmed || state.preview.items.length === 0 || state.preview.issues.length > 0}
      onClick={() => void state.execute()}>{t(state.stage === "execute" ? "sessions.range.executing" : "sessions.range.confirm")}</Button>
      : <Button variant="primary" disabled={state.busy || !state.valid} onClick={() => void state.prepare()}>
        {t(state.stage === "preview" ? "sessions.range.preparing" : "sessions.range.preview")}</Button>}
  </>}>
    <RangeForm state={state} app={app} archived={archived} scopeRef={scope} />
    {state.error && <p className="asb-warn-text" role="alert">{state.error}</p>}
    {state.preview && !state.progress && <RangePreview key={state.preview.id} preview={state.preview} />}
    {state.preview && state.stage !== "execute" && <label className="asb-session-range-confirm"><input type="checkbox"
      checked={state.confirmed} disabled={state.busy} onChange={(event) => state.setConfirmed(event.target.checked)} />
      {t(state.action === "delete" ? "sessions.range.confirmDelete" : "sessions.range.confirmArchive")}</label>}
    {state.progress && <RangeResults progress={{ ...state.progress, running: state.stage === "execute" }} action={state.resultAction} />}
  </AppDialog>;
}
