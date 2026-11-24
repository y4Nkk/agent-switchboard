import { useState } from "react";
import { useI18n } from "../../i18n";
import { AppDialog } from "../AppDialog";
import { Checkbox } from "../Checkbox";
import { Collapsible } from "../Collapsible";
import { Select } from "../Select";
import { SessionDateFields } from "./SessionDateFields";
import { sessionDatesInOrder, type SessionDates } from "./session-dates";
import type { SessionSearch } from "./use-session-search";

function SessionTimeFilter({ search, disabled }: { search: SessionSearch; disabled: boolean }) {
  const { t } = useI18n();
  const { range, dates } = search.timeFilter;
  const custom = range === "custom";
  const [invalidDraft, setInvalidDraft] = useState<SessionDates | null>(null);
  const options = [
    { value: "all", label: t("sessions.time.all") },
    { value: "7", label: t("sessions.time.week") },
    { value: "30", label: t("sessions.time.month") },
    { value: "custom", label: t("sessions.time.custom") },
  ] as const;
  const changeRange = (value: string) => {
    const option = options.find((item) => item.value === value);
    if (!option) return;
    setInvalidDraft(null);
    search.changeTime(option.value, dates);
  };
  const changeDates = (next: SessionDates) => {
    if (!sessionDatesInOrder(next)) { setInvalidDraft(next); return; }
    setInvalidDraft(null);
    search.changeTime("custom", next);
  };
  return <div>
    <div className="asb-field">
      <span>{t("sessions.time.range")}</span>
      <Select value={range} ariaLabel={t("sessions.time.range")} disabled={disabled} onChange={changeRange} options={options} />
    </div>
    <Collapsible open={custom}>
      <div className="asb-session-date-range">
        <SessionDateFields dates={invalidDraft ?? dates} disabled={disabled || !custom} onChange={changeDates} />
      </div>
      {invalidDraft && <p className="asb-field-error" role="alert">{t("sessions.time.invalid")}</p>}
    </Collapsible>
  </div>;
}

export function SessionOrganizationFilters({ search, busy, onClose }: {
  search: SessionSearch; busy: boolean; onClose: () => void;
}) {
  const { t } = useI18n();
  return <AppDialog title={t("sessions.search.filters")} busy={busy} onClose={onClose} narrow>
    <SessionTimeFilter search={search} disabled={busy} />
    <div className="asb-field">
      <span>{t("sessions.organize.tags")}</span>
      <Select value={search.tag === null ? "all" : `tag:${search.tag}`} ariaLabel={t("sessions.organize.tagFilter")}
        disabled={busy} onChange={(value) => search.changeTag(value === "all" ? null : value.slice(4))}
        options={[{ value: "all", label: t("sessions.organize.allTags") },
          ...[...new Set([...search.tags, ...(search.tag ? [search.tag] : [])])].map((tag) => ({ value: `tag:${tag}`, label: tag }))]} />
    </div>
    <Checkbox label={t("sessions.organize.pinnedOnly")} checked={search.pinnedOnly} disabled={busy}
      onChange={search.changePinned} />
  </AppDialog>;
}
