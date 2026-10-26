import { useRef, useState } from "react";
import { Copy } from "lucide-react";
import { ActionMenu } from "../ActionMenu";
import { exportSessionMarkdown, resumeSession, type SessionMeta } from "../../api/client";
import { useI18n, type MessageKey } from "../../i18n";
import { uiMessage } from "../../i18n/errors";
import { useMessageState } from "../../i18n/use-message-state";
import { clientFullName } from "../../lib/client-name";
import { Button } from "../Button";
import { ClientLogo } from "../ClientLogo";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { Time } from "../Time";
import { ScrollArea } from "../ScrollArea";
import { copyText, sessionTitle } from "./session-content";
import { SessionOrganizationEditor } from "./SessionOrganizationEditor";
import type { SessionOrganization } from "./use-session-organization";
import { SessionTranscript } from "./SessionTranscript";
import type { SessionDetailState } from "./use-session-detail";
import type { SessionBookmarks } from "./use-session-bookmarks";

function useDetailActions(session: SessionMeta) {
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const [status, setStatus] = useMessageState();
  const [error, setError] = useMessageState();
  const run = async (action: "resume" | "export") => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setStatus(null); setError(null);
    try {
      if (action === "resume") {
        const result = await resumeSession(session.app, session.sessionId);
        setStatus(uiMessage(result.usedProjectDir ? "sessions.resume.done" : "sessions.resume.doneFallback"));
      } else {
        const path = await exportSessionMarkdown(session.app, session.sessionId);
        if (path) setStatus(uiMessage("sessions.export.done", { path }));
      }
    } catch (caught) { setError(caught); }
    finally { lock.current = false; setBusy(false); }
  };
  const copy = async (text: string, labelKey: MessageKey) => {
    setError(null);
    try { await copyText(text); setStatus(uiMessage("sessions.status.copied", { labelKey })); }
    catch { setError(uiMessage("sessions.status.copyFailed", { labelKey })); }
  };
  return { busy, status, error, run, copy };
}

function SessionFacts({ session, copy }: { session: SessionMeta; copy: (text: string, labelKey: MessageKey) => Promise<void> }) {
  const { t } = useI18n();
  const resumeCommand = session.app === "codex" ? session.resumeCommand : null;
  return <FactList rows={[
    { label: t("sessions.fact.sessionId"), value: <code>{session.sessionId}</code> },
    { label: t("sessions.fact.lastActive"), value: session.lastActiveAt
      ? <Time iso={session.lastActiveAt} /> : t("sessions.time.unknown") },
    ...(session.projectDir ? [{ label: t("sessions.label.projectDir"), value:
      <Button variant="unstyled" className="asb-fact-path"
        title={t("sessions.projectDir.copyHint", { dir: session.projectDir })}
        onClick={() => void copy(session.projectDir!, "sessions.label.projectDir")}>{session.projectDir}</Button> }] : []),
    ...(resumeCommand ? [{ label: t("sessions.label.resumeCommand"), value: <span className="asb-session-resume-fact">
      <code className="asb-code" title={resumeCommand}>{resumeCommand}</code>
      <Button variant="icon" aria-label={t("sessions.action.copy", { label: t("sessions.label.resumeCommand") })}
        title={t("sessions.action.copy", { label: t("sessions.label.resumeCommand") })}
        onClick={() => void copy(resumeCommand, "sessions.label.resumeCommand")}><Copy aria-hidden="true" /></Button>
    </span> }] : []),
    ...(session.preservation ? [{ label: t("sessions.preservation.saved"), value: <Time iso={session.preservation.savedAt} /> }] : []),
  ]} />;
}

function SessionMoreActions({ session, organization, disabled, onExport, onOrganize, onDelete }: {
  session: SessionMeta; organization: SessionOrganization; disabled: boolean;
  onExport: () => void; onOrganize: () => void; onDelete: () => void;
}) {
  const { t } = useI18n();
  return <ActionMenu label={t("sessions.actions.more")} ariaLabel={t("sessions.actions.moreAria")}
    disabled={disabled} items={[
      { id: "export", label: t("sessions.export.action"), onAction: onExport },
      { id: "organize", label: t("sessions.organize.heading"), onAction: onOrganize },
      { id: "pin", label: t(session.pinned ? "sessions.organize.unpin" : "sessions.organize.pin"),
        onAction: () => void organization.save([session], { kind: "pin", pinned: !session.pinned }) },
      { id: "delete", label: t("sessions.delete.title"), danger: true,
        disabled: !session.canDelete, onAction: onDelete },
    ]} />;
}

export function SessionDetail({ detail, session, bookmarks, deleting, onDelete, organization }: {
  detail: SessionDetailState; session: SessionMeta; bookmarks: SessionBookmarks;
  organization: SessionOrganization;
  deleting: boolean; onDelete: (session: SessionMeta) => void;
}) {
  const { t } = useI18n();
  const actions = useDetailActions(session);
  const [organizing, setOrganizing] = useState(false);
  const [informationOpen, setInformationOpen] = useState(false);
  return <ScrollArea as="section" className="asb-session-detail" aria-label={t("sessions.detail.aria")}>
    <header className="asb-session-detail-head">
      <div className="asb-session-detail-title">
        <span className="asb-session-client"><ClientLogo app={session.app} className="asb-session-logo" />
          {clientFullName(session.app)}</span><h3 className="asb-section-title" title={sessionTitle(session)}>{sessionTitle(session)}</h3>
      </div>
      <div className="asb-session-actions">
        <Button variant="primary" disabled={actions.busy || deleting} onClick={() => void actions.run("resume")}>{t("sessions.resume.action")}</Button>
        <SessionMoreActions session={session} organization={organization}
          disabled={actions.busy || organization.busy || deleting || detail.busy}
          onExport={() => void actions.run("export")} onOrganize={() => setOrganizing(true)}
          onDelete={() => onDelete(session)} />
      </div>
    </header>
    <CollapsibleSection quiet open={informationOpen}
      onOpenChange={setInformationOpen} label={t("sessions.detail.information")}>
      <div className="asb-session-information-content">
        <SessionFacts session={session} copy={actions.copy} />
        {session.preservation && <p className="asb-scope-note">{t("sessions.preservation.resume")}</p>}
        {session.preservation && !session.preservation.sourceAvailable && <p className="asb-scope-note">{t("sessions.preservation.sourceMissing")}</p>}
        {!session.canDelete && <p className="asb-scope-note">{t("sessions.nativeHistoryManaged")}</p>}
      </div>
    </CollapsibleSection>
    {session.preservation?.needsAttention && <p className="asb-warn-text" role="status">{t("sessions.preservation.attention")}</p>}
    {organizing && <SessionOrganizationEditor session={session} organization={organization}
      disabled={deleting || detail.busy} onClose={() => setOrganizing(false)} />}
    {actions.status && <p className="asb-scope-note" role="status">{actions.status}</p>}
    {actions.error && <p className="asb-warn-text" role="alert">{actions.error}</p>}
    {detail.busy && <p role="status">{t("sessions.transcript.loading.aria")}</p>}
    {detail.error && <p className="asb-warn-text" role="alert">{detail.error}</p>}
    {detail.page && <SessionTranscript key={`${session.app}:${session.sessionId}:${detail.page.offset}`}
      session={session} detail={detail} bookmarks={bookmarks} />}
  </ScrollArea>;
}
