import { useEffect, useId, useState } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { SessionTagInput } from "./SessionTagInput";
import type { SessionDeletionState } from "./SessionDeletion";
import type { SessionSearch } from "./use-session-search";
import type { SessionOrganization } from "./use-session-organization";

export function SessionSelectionToolbar({ search, selection, organization, disabled: externalBusy = false }: {
  search: SessionSearch; selection: SessionDeletionState; organization: SessionOrganization;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const tagPanelId = useId();
  const [tagging, setTagging] = useState(false);
  const [tags, setTags] = useState<string[]>([]);
  const sessions = [...selection.chosen.values()];
  const busy = selection.busy || organization.busy || externalBusy;
  const overLimit = sessions.length > 200;
  const disabled = busy || !sessions.length || overLimit;
  useEffect(() => {
    if (sessions.length === 0) { setTagging(false); setTags([]); }
  }, [sessions.length]);
  return <div className="asb-session-selection">
    <div className="asb-session-selection-row">
      <strong className="asb-session-selection-count">{t("sessions.batch.selectedCount", { count: sessions.length })}</strong>
      <Button variant="unstyled" className="asb-session-quiet-action"
        disabled={search.busy || busy || !search.result?.results.length}
        onClick={() => selection.choosePage(search.result?.results.map((hit) => hit.session) ?? [])}>
        {t("sessions.batch.selectPage")}
      </Button>
      <div className="asb-session-selection-actions">
        <Button variant="unstyled" className="asb-session-quiet-action" disabled={disabled}
          onClick={() => void organization.save(sessions, { kind: "pin", pinned: true })}>{t("sessions.organize.pin")}</Button>
        <Button variant="unstyled" className="asb-session-quiet-action" disabled={disabled}
          onClick={() => void organization.save(sessions, { kind: "pin", pinned: false })}>{t("sessions.organize.unpin")}</Button>
        <Button variant="unstyled" className="asb-session-quiet-action" disabled={disabled}
          aria-expanded={tagging} aria-controls={tagging ? tagPanelId : undefined} onClick={() => setTagging(!tagging)}>
          {t("sessions.organize.tags")}
        </Button>
        <Button variant="unstyled" className="asb-session-quiet-action asb-session-selection-delete"
          disabled={busy || !sessions.length || sessions.some((session) => !session.canDelete)} onClick={() => selection.setPending(sessions)}>
          {t("sessions.batch.deleteSelected", { count: sessions.length })}
        </Button>
        <Button variant="unstyled" className="asb-session-quiet-action" disabled={busy}
          onClick={selection.toggleMode}>{t("sessions.batch.exit")}</Button>
      </div>
    </div>
    <Collapsible open={tagging} id={tagPanelId}>
      <div className="asb-session-selection-tags">
        <SessionTagInput tags={tags} disabled={disabled} onChange={setTags} />
        <Button variant="secondary" disabled={disabled || !tags.length}
          onClick={() => void organization.save(sessions, { kind: "addTags", tags })}>{t("sessions.organize.addTags")}</Button>
        <Button variant="secondary" disabled={disabled || !tags.length}
          onClick={() => void organization.save(sessions, { kind: "removeTags", tags })}>{t("sessions.organize.removeTags")}</Button>
      </div>
    </Collapsible>
    {overLimit && <p className="asb-warn-text">{t("sessions.organize.batchLimit")}</p>}
  </div>;
}
