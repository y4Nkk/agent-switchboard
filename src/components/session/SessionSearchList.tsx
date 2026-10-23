import { Fragment } from "react";
import type { SessionSearchHit, SessionMeta } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientFullName } from "../../lib/client-name";
import { Button } from "../Button";
import { ClientLogo } from "../ClientLogo";
import { Pagination } from "../Pagination";
import { Time } from "../Time";
import { ScrollArea } from "../ScrollArea";
import { SessionProjects } from "./SessionProjects";
import { SessionSearchProgress } from "./SessionSearchProgress";
import { sessionKey, sessionTitle } from "./session-content";
import { SESSION_PAGE_SIZE, type SessionSearch } from "./use-session-search";

interface Props {
  search: SessionSearch;
  selected: SessionMeta | null;
  selecting: boolean;
  chosen: ReadonlyMap<string, SessionMeta>;
  disabled: boolean;
  onSelect: (hit: SessionSearchHit) => void;
  onToggle: (session: SessionMeta) => void;
  onToggleMode: () => void;
}

function SearchRow({ hit, active, picked, selecting, disabled, onClick }: {
  hit: SessionSearchHit; active: boolean; picked: boolean; selecting: boolean;
  disabled: boolean; onClick: () => void;
}) {
  const { t } = useI18n();
  const { session } = hit;
  return <div className="asb-session-result-row"><Button variant="unstyled" disabled={disabled} onClick={onClick}
    aria-pressed={selecting ? picked : active}
    className={`asb-session-item${active ? " is-active" : ""}${picked ? " is-selected" : ""}`}>
    <span className="asb-session-item-title">
      <ClientLogo app={session.app} className="asb-session-logo" />
      <span title={sessionTitle(session)}>{sessionTitle(session)}</span>
    </span>
    {(session.pinned || session.tags.length > 0) && <span className="asb-session-labels">
      {session.pinned && <span>{t("sessions.organize.pinned")}</span>}
      {session.tags.map((tag) => <span key={tag}>#{tag}</span>)}
    </span>}
    <span className="asb-session-search-excerpt">{hit.excerpt}</span>
    {hit.messageId && <span className="asb-session-item-time">{t("sessions.search.messageHit")}</span>}
    <span className="asb-session-item-time">
      {session.lastActiveAt ? <Time iso={session.lastActiveAt} /> : t("sessions.time.unknown")}
    </span>
  </Button></div>;
}

export function SessionSearchList({ search, selected, selecting, chosen, disabled, onSelect, onToggle, onToggleMode }: Props) {
  const { t } = useI18n();
  const { result, busy, error } = search;
  return <section className="asb-session-list" aria-label={t("sessions.list.aria")} aria-busy={busy}>
    <div className="asb-session-list-heading">
      <span>{t(search.query ? "sessions.search.results" : "sessions.list.heading")}</span>
      <div className="asb-session-list-heading-actions">
        {result && <span className="asb-session-count">{result.total}</span>}
        {!selecting && <Button variant="unstyled" className="asb-session-quiet-action" disabled={disabled}
          onClick={onToggleMode}>{t("sessions.batch.enter")}</Button>}
      </div>
    </div>
    <SessionProjects search={search} disabled={disabled} />
    {busy && <SessionSearchProgress progress={search.progress} />}
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    {result?.total === 0 && <p className="asb-empty-state">{t("sessions.list.empty")}</p>}
    <ScrollArea className="asb-session-items" aria-label={t("sessions.list.aria")}>
      {result?.results.map((hit, index) => {
        const previous = result.results[index - 1]?.session;
        const firstInProject = !search.project && (!previous || previous.app !== hit.session.app
          || previous.projectDir !== hit.session.projectDir);
        const dir = hit.session.projectDir ?? t("sessions.projects.none");
        return <Fragment key={`${sessionKey(hit.session)}:${hit.messageId ?? "metadata"}`}>
          {firstInProject && <Button variant="unstyled" className="asb-session-project-heading"
            disabled={disabled} onClick={() => search.openProject({ app: hit.session.app, projectDir: hit.session.projectDir })}
            title={t("sessions.projects.openHint", { dir })}>
            {clientFullName(hit.session.app)} · {dir}
          </Button>}
          <SearchRow hit={hit} active={selected !== null && sessionKey(selected) === sessionKey(hit.session)}
            picked={chosen.has(sessionKey(hit.session))} selecting={selecting} disabled={disabled}
            onClick={() => selecting ? onToggle(hit.session) : onSelect(hit)} />
        </Fragment>;
      })}
    </ScrollArea>
    {result && <Pagination total={result.total} page={search.page} pageSize={SESSION_PAGE_SIZE}
      onPageChange={search.setPage} label={t("sessions.search.pagination")} />}
  </section>;
}
