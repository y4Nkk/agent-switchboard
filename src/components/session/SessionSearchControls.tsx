import { ListFilter } from "lucide-react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Tooltip } from "../Tooltip";
import { SessionProjects } from "./SessionProjects";
import type { SessionSearch } from "./use-session-search";

export function SessionSearchControls({ search, disabled, filtersOpen, onOpenFilters }: {
  search: SessionSearch; disabled: boolean; filtersOpen: boolean; onOpenFilters: () => void;
}) {
  const { t } = useI18n();
  const filtersLabel = t(search.filtersActive ? "sessions.search.filtersActive" : "sessions.search.filters");
  return <div className="asb-session-search-controls">
    <SessionProjects search={search} disabled={disabled} />
    <div className="asb-session-search-field">
      <textarea className="asb-input asb-session-search-query" rows={1} value={search.draft} disabled={disabled}
        aria-label={t("sessions.search.aria")} placeholder={t("sessions.search.placeholder")}
        onChange={(event) => search.setDraft(event.target.value)}
        onCompositionStart={() => search.setComposing(true)}
        onCompositionEnd={(event) => {
          search.setDraft(event.currentTarget.value);
          search.setComposing(false);
        }}
        onBlur={() => search.setComposing(false)} />
      <Tooltip label={filtersLabel} side="bottom">
        <Button variant="icon"
          className={`asb-session-toolbar-button asb-session-filter-button${search.filtersActive ? " is-active" : ""}`}
          disabled={disabled} aria-label={filtersLabel} aria-haspopup="dialog" aria-expanded={filtersOpen}
          onClick={onOpenFilters}><ListFilter aria-hidden="true" /></Button>
      </Tooltip>
    </div>
  </div>;
}
