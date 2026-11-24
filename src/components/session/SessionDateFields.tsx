import { useI18n } from "../../i18n";
import { DatePicker } from "../DatePicker";
import type { SessionDates } from "./session-dates";

export function SessionDateFields({ dates, disabled, onChange }: {
  dates: SessionDates; disabled: boolean; onChange: (dates: SessionDates) => void;
}) {
  const { t } = useI18n();
  return <>
    <label className="asb-field">{t("sessions.time.from")}
      <DatePicker value={dates.from} disabled={disabled} ariaLabel={t("sessions.time.from")}
        onChange={(from) => onChange({ ...dates, from })} />
    </label>
    <label className="asb-field">{t("sessions.time.to")}
      <DatePicker value={dates.to} disabled={disabled} ariaLabel={t("sessions.time.to")}
        onChange={(to) => onChange({ ...dates, to })} />
    </label>
  </>;
}
