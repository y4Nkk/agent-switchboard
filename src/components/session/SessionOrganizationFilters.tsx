import { useI18n } from "../../i18n";
import { Select } from "../Select";
import { DatePicker } from "../DatePicker";
import type { SessionSearch } from "./use-session-search";

import { dateBoundary } from "./session-dates";

function SessionTimeFilter({ search, disabled }: { search: SessionSearch; disabled: boolean }) {
  const { t } = useI18n();
  const { range, from, to } = search.timeDraft;
  const apply = (start: string, end: string) => {
    if (!start || !end || start <= end) {
      search.changeDates({ activeAfter: dateBoundary(start, false), activeBefore: dateBoundary(end, true) });
    }
  };
  const change = (value: string) => {
    search.setTimeDraft({ range: value, from: value === "all" ? "" : from, to: value === "all" ? "" : to });
    if (value === "custom") {
      apply(from, to);
    } else search.changeDates({ activeAfter: value === "all" ? null
      : new Date(Date.now() - Number(value) * 86400000).toISOString(), activeBefore: null });
  };
  const update = (start: string, end: string) => {
    search.setTimeDraft({ range, from: start, to: end });
    apply(start, end);
  };
  return <>
    <Select value={range} ariaLabel={t("sessions.time.range")} disabled={disabled} onChange={change} options={[
      { value: "all", label: t("sessions.time.all") }, { value: "7", label: t("sessions.time.week") },
      { value: "30", label: t("sessions.time.month") }, { value: "custom", label: t("sessions.time.custom") },
    ]} />
    {range === "custom" && <div className="asb-session-date-range">
      <label>{t("sessions.time.from")}<DatePicker value={from} disabled={disabled}
        ariaLabel={t("sessions.time.from")} onChange={(value) => update(value, to)} /></label>
      <label>{t("sessions.time.to")}<DatePicker value={to} disabled={disabled}
        ariaLabel={t("sessions.time.to")} onChange={(value) => update(from, value)} /></label>
    </div>}
  </>;
}

export function SessionOrganizationFilters({ search, disabled }: { search: SessionSearch; disabled: boolean }) {
  const { t } = useI18n();
  const { range, from, to } = search.timeDraft;
  const invalid = range === "custom" && Boolean(from && to && from > to);
  return <>
    <SessionTimeFilter search={search} disabled={disabled} />
    <Select value={search.tag === null ? "all" : `tag:${search.tag}`} ariaLabel={t("sessions.organize.tagFilter")}
      disabled={disabled} onChange={(value) => search.changeTag(value === "all" ? null : value.slice(4))}
      options={[{ value: "all", label: t("sessions.organize.allTags") },
        ...[...new Set([...search.tags, ...(search.tag ? [search.tag] : [])])].map((tag) => ({ value: `tag:${tag}`, label: tag }))]} />
    <label className="asb-session-pin-filter"><input type="checkbox" checked={search.pinnedOnly} disabled={disabled}
      onChange={(event) => search.changePinned(event.target.checked)} />{t("sessions.organize.pinnedOnly")}</label>
    {invalid && <p className="asb-field-error asb-session-filter-error" role="alert">
      {t("sessions.time.invalid")}
    </p>}
  </>;
}
