import { useState } from "react";
import type { SessionMeta, SessionSearchHit } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientFullName } from "../../lib/client-name";
import { AppDialog, AppDialogSuspension } from "../AppDialog";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { ClientLogo } from "../ClientLogo";
import { Pagination } from "../Pagination";
import { Time } from "../Time";
import { ToastMessageList, ToastText } from "../ToastContent";
import { SessionDeletionConfirm, type SessionDeletionState } from "./SessionDeletion";
import { SessionOrganizationFilters } from "./SessionOrganizationFilters";
import { SessionSearchControls } from "./SessionSearchControls";
import { SessionSearchProgress } from "./SessionSearchProgress";
import { SessionTagInput } from "./SessionTagInput";
import { sessionKey, sessionTitle } from "./session-content";
import { SESSION_ORGANIZATION_BATCH_LIMIT, type SessionOrganization } from "./use-session-organization";
import { SESSION_PAGE_SIZE, type SessionSearch } from "./use-session-search";
import type { SessionSelection } from "./use-session-selection";

function SessionBatchChoices({ search, selection, disabled }: {
  search: SessionSearch; selection: SessionSelection; disabled: boolean;
}) {
  const { t } = useI18n();
  const unique = new Map<string, SessionSearchHit>();
  for (const hit of search.result?.results ?? []) {
    const key = sessionKey(hit.session);
    if (!unique.has(key)) unique.set(key, hit);
  }
  const rows = [...unique.values()];
  const chosenCount = rows.filter(({ session }) => selection.chosen.has(sessionKey(session))).length;
  return <div className="asb-session-batch-choices" aria-busy={search.busy}>
    <Checkbox label={t("sessions.batch.selectPage")} checked={rows.length > 0 && chosenCount === rows.length}
      indeterminate={chosenCount > 0 && chosenCount < rows.length}
      disabled={disabled || search.busy || rows.length === 0}
      onChange={(checked) => selection.choose(rows.map(({ session }) => session), checked)} />
    {search.busy && <SessionSearchProgress progress={search.progress} />}
    {search.error && <p className="asb-warn-text" role="alert">{search.error}</p>}
    {search.result?.total === 0 && <p className="asb-empty-state">{t("sessions.list.empty")}</p>}
    {rows.length > 0 && <ul className="asb-session-batch-list" aria-label={t("sessions.list.aria")}>
      {rows.map(({ session, excerpt }) => {
        const key = sessionKey(session);
        const title = sessionTitle(session);
        const checked = selection.chosen.has(key);
        return <li key={key} data-selected={checked || undefined}>
          <div className="asb-session-batch-choice" title={title}>
            <Checkbox label={title} ariaLabel={`${title} · ${clientFullName(session.app)}`}
              checked={checked} disabled={disabled} onChange={(next) => selection.choose([session], next)} />
            {search.query && excerpt && <span className="asb-session-batch-excerpt">{excerpt}</span>}
          </div>
          <span className="asb-session-batch-session-meta" title={session.projectDir ?? t("sessions.projects.none")}>
            <ClientLogo app={session.app} className="asb-session-logo" />
            {session.lastActiveAt ? <Time iso={session.lastActiveAt} /> : t("sessions.time.unknown")}
          </span>
        </li>;
      })}
    </ul>}
  </div>;
}

function SessionBatchTagsDialog({ sessions, organization, onClose }: {
  sessions: SessionMeta[]; organization: SessionOrganization; onClose: () => void;
}) {
  const { t } = useI18n();
  const [tags, setTags] = useState<string[]>([]);
  const disabled = organization.busy || tags.length === 0;
  return <AppDialog title={t("sessions.batch.tagsTitle")} busy={organization.busy} onClose={onClose} narrow footer={<>
    <Button variant="secondary" disabled={disabled}
      onClick={() => void organization.save(sessions, { kind: "removeTags", tags })}>{t("sessions.organize.removeTags")}</Button>
    <Button variant="primary" disabled={disabled}
      onClick={() => void organization.save(sessions, { kind: "addTags", tags })}>{t("sessions.organize.addTags")}</Button>
  </>}>
    <p className="asb-scope-note">{t("sessions.batch.selectedCount", { count: sessions.length })}</p>
    <SessionTagInput tags={tags} disabled={organization.busy} onChange={setTags} />
    {organization.error && <p className="asb-warn-text" role="alert">{organization.error}</p>}
    {organization.status && <p className="asb-scope-note" role="status">{organization.status}</p>}
  </AppDialog>;
}

function SessionBatchFooter({ search, sessions, deletion, organization, busy, onTags }: {
  search: SessionSearch; sessions: SessionMeta[]; deletion: SessionDeletionState;
  organization: SessionOrganization; busy: boolean; onTags: () => void;
}) {
  const { t } = useI18n();
  const count = sessions.length;
  const overLimit = count > SESSION_ORGANIZATION_BATCH_LIMIT;
  const organizeDisabled = busy || count === 0 || overLimit;
  const deleteDisabled = busy || count === 0 || sessions.some((session) => !session.canDelete);
  return <div className="asb-session-batch-footer">
    <div className="asb-session-batch-summary">
      <strong role="status">{t("sessions.batch.selectedCount", { count })}</strong>
      {search.total !== null && <Pagination total={search.total} page={search.page} pageSize={SESSION_PAGE_SIZE}
        onPageChange={search.setPage} label={t("sessions.search.pagination")} />}
    </div>
    {overLimit && <p className="asb-warn-text">{t("sessions.organize.batchLimit")}</p>}
    {organization.status && <p className="asb-scope-note" role="status">{organization.status}</p>}
    {deletion.feedback?.deleted && <p className="asb-scope-note" role="status">
      <ToastText content={deletion.feedback.deleted} />
    </p>}
    {deletion.feedback?.failed && <p className="asb-warn-text" role="alert">
      <ToastText content={deletion.feedback.failed.title} />
    </p>}
    <div className="asb-session-batch-actions">
      <Button variant="secondary" disabled={organizeDisabled}
        onClick={() => void organization.save(sessions, { kind: "pin", pinned: true })}>{t("sessions.organize.pin")}</Button>
      <Button variant="secondary" disabled={organizeDisabled}
        onClick={() => void organization.save(sessions, { kind: "pin", pinned: false })}>{t("sessions.organize.unpin")}</Button>
      <Button variant="secondary" disabled={organizeDisabled} aria-haspopup="dialog" onClick={onTags}>
        {t("sessions.organize.tags")}
      </Button>
      <Button variant="danger" disabled={deleteDisabled} aria-haspopup="dialog" onClick={() => deletion.setPending(sessions)}>
        {t("sessions.batch.deleteSelected", { count })}
      </Button>
    </div>
  </div>;
}

export function SessionBatchDialog({ search, selection, deletion, organization, onClose }: {
  search: SessionSearch; selection: SessionSelection; deletion: SessionDeletionState;
  organization: SessionOrganization; onClose: () => void;
}) {
  const { t } = useI18n();
  const [panel, setPanel] = useState<"filters" | "tags" | null>(null);
  const sessions = [...selection.chosen.values()];
  const busy = deletion.busy || organization.busy;
  const child = deletion.pending.length > 0 ? "delete" : panel;
  return <>
    <AppDialogSuspension suspended={child !== null}>
      <AppDialog title={t("sessions.batch.title")} busy={busy} onClose={onClose} tall footer={
        <SessionBatchFooter search={search} sessions={sessions} deletion={deletion} organization={organization} busy={busy}
          onTags={() => { organization.resetFeedback(); setPanel("tags"); }} />
      }>
        <SessionSearchControls search={search} disabled={busy}
          filtersOpen={child === "filters"} onOpenFilters={() => setPanel("filters")} />
        {organization.error && <p className="asb-warn-text" role="alert">{organization.error}</p>}
        {deletion.error && <p className="asb-warn-text" role="alert">{deletion.error}</p>}
        {deletion.feedback?.failed && <div className="asb-session-batch-issues asb-warn-text">
          <ToastMessageList items={deletion.feedback.failed.items} />
        </div>}
        <SessionBatchChoices search={search} selection={selection} disabled={busy} />
      </AppDialog>
    </AppDialogSuspension>
    {child === "filters" && <SessionOrganizationFilters search={search} busy={busy} onClose={() => setPanel(null)} />}
    {child === "tags" && <SessionBatchTagsDialog sessions={sessions} organization={organization} onClose={() => setPanel(null)} />}
    {child === "delete" && <SessionDeletionConfirm state={deletion} disabled={organization.busy} />}
  </>;
}
