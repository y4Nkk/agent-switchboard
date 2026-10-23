import type { SessionMeta } from "../../api/client";
import { useI18n } from "../../i18n";
import { SessionDetail } from "./SessionDetail";
import { ScrollArea } from "../ScrollArea";
import { SessionSearchList } from "./SessionSearchList";
import { SessionSelectionToolbar } from "./SessionSelectionToolbar";
import { SessionSourceNotices } from "./SessionSourceNotices";
import type { SessionConversion } from "./use-session-conversion";
import { sessionKey } from "./session-content";
import type { SessionSearch } from "./use-session-search";
import type { SessionDetailState } from "./use-session-detail";
import type { SessionBookmarks } from "./use-session-bookmarks";
import type { SessionOrganization } from "./use-session-organization";
import type { SessionDeletionState } from "./SessionDeletion";

export function SessionHistory({ search, detail, bookmarks, organization, deletion, conversion, onOpen }: {
  search: SessionSearch; detail: SessionDetailState; bookmarks: SessionBookmarks;
  organization: SessionOrganization; deletion: SessionDeletionState;
  conversion: SessionConversion;
  onOpen: (session: SessionMeta, messageId: string | null, contentOffset: number) => void;
}) {
  const { t } = useI18n();
  const busy = deletion.busy || organization.busy || conversion.busy;
  return <>
    {deletion.selecting && <SessionSelectionToolbar search={search} selection={deletion} organization={organization} disabled={conversion.busy} />}
    <SessionSourceNotices issues={search.issues} conversion={conversion} disabled={busy} />
    <div className="asb-session-layout">
      <SessionSearchList search={search} selected={detail.selected} selecting={deletion.selecting}
        chosen={deletion.chosen} disabled={busy} onToggle={deletion.toggle}
        onToggleMode={deletion.toggleMode}
        onSelect={(hit) => onOpen(hit.session, hit.messageId, hit.contentOffset)} />
      {detail.selected ? <SessionDetail key={sessionKey(detail.selected)} detail={detail} session={detail.selected}
        bookmarks={bookmarks} organization={organization} deleting={busy} onDelete={(session) => deletion.setPending([session])} />
        : <ScrollArea as="section" className="asb-session-detail asb-session-empty-detail" aria-label={t("sessions.detail.aria")}>
          <p>{t("sessions.detail.empty")}</p>
        </ScrollArea>}
    </div>
  </>;
}
