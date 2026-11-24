import { createCalendar, isSameMonth, isToday, parseDate, type CalendarDate } from "@internationalized/date";
import { useButton, useCalendar, useCalendarCell, useCalendarGrid, useFocusRing, I18nProvider,
  mergeProps, useLocale } from "react-aria";
import { useCalendarState } from "react-stately";
import { useContext, useEffect, useRef } from "react";
import { Dialog, Popover } from "react-aria-components";
import { useI18n } from "../i18n";
import { dateLabel } from "../lib/time";
import { useDialogFieldPopup } from "./AppDialog";
import { Button } from "./Button";
import { ChevronDownIcon, ChevronLeftIcon, ChevronRightIcon } from "./icons";
import { OverlayMotionContext } from "./overlay-motion";

interface Props {
  /** Selected calendar date as `YYYY-MM-DD`; empty = nothing chosen yet. */
  value: string;
  /** Emits the newly chosen `YYYY-MM-DD`. */
  onChange: (value: string) => void;
  /** Accessible name of the field trigger and its calendar dialog. */
  ariaLabel: string;
  disabled?: boolean;
}

/**
 * Calendar date picker: a field-shaped trigger opening a month calendar
 * popover. Selection state and behavior (grid keyboard navigation, aria
 * roles, localized weekday/month names and locale week start) come from the
 * react-aria calendar hooks; every DOM structure — trigger, table, cells —
 * is owned here. Visuals live in date-picker.css from tokens.css, and the
 * chosen date renders through the shared `dateLabel` like every timestamp.
 */
export function DatePicker({ value, onChange, ariaLabel, disabled = false }: Props) {
  const { language, t } = useI18n();
  const animate = useContext(OverlayMotionContext);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popup = useDialogFieldPopup(disabled);
  const close = () => popup.onOpenChange(false);
  const selected = value ? parseDate(value) : null;
  return <I18nProvider locale={language}>
    <Button variant="unstyled" ref={triggerRef} className="asb-select-trigger asb-date-trigger"
      aria-haspopup="dialog" aria-expanded={popup.open} aria-label={ariaLabel} disabled={popup.disabled}
      onClick={() => popup.onOpenChange(!popup.open)}>
      <span className="asb-date-value" data-placeholder={!selected || undefined}>
        {selected ? dateLabel(selected.toString()) : t("format.date.unset")}
      </span>
      <span className="asb-select-chevron" aria-hidden="true"><ChevronDownIcon /></span>
    </Button>
    {popup.open && <Popover triggerRef={triggerRef} isOpen UNSTABLE_portalContainer={popup.container}
      onOpenChange={popup.onOpenChange}
      placement="bottom start" offset={4} className="asb-date-popover" data-overlay-motion={animate || undefined}>
      <Dialog aria-label={ariaLabel} className="asb-date-dialog">
        <CalendarSheet selected={selected} disabled={disabled} ariaLabel={ariaLabel}
          onSelect={(date) => { onChange(date.toString()); close(); }} />
        <div className="asb-date-actions">
          <Button variant="secondary" disabled={disabled || !selected}
            onClick={() => { onChange(""); close(); }}>{t("format.date.clear")}</Button>
        </div>
      </Dialog>
    </Popover>}
  </I18nProvider>;
}

function CalendarSheet({ selected, disabled, ariaLabel, onSelect }: {
  selected: CalendarDate | null;
  disabled: boolean;
  ariaLabel: string;
  onSelect: (date: CalendarDate) => void;
}) {
  const { locale } = useLocale();
  const state = useCalendarState({
    locale, createCalendar, value: selected, onChange: onSelect, isDisabled: disabled,
  });
  // Passing the label here also names the grid through the hook's state data.
  const { calendarProps, prevButtonProps, nextButtonProps, title } = useCalendar({ "aria-label": ariaLabel }, state);
  const { gridProps, headerProps, weekDays, weeksInMonth } = useCalendarGrid({}, state);
  const prevRef = useRef<HTMLButtonElement>(null);
  const nextRef = useRef<HTMLButtonElement>(null);
  const prev = useButton(prevButtonProps, prevRef);
  const next = useButton(nextButtonProps, nextRef);
  const rowsRef = useRef<HTMLTableSectionElement>(null);
  // The focused date is the one roving-tabindex cell; it receives focus on open.
  useEffect(() => {
    rowsRef.current?.querySelector<HTMLElement>('[tabindex="0"]')?.focus();
  }, []);
  const month = state.visibleRange.start;
  return <div {...calendarProps} className="asb-calendar">
    <div className="asb-calendar-header">
      <button {...prev.buttonProps} ref={prevRef} className="asb-calendar-nav"><ChevronLeftIcon /></button>
      <h2 className="asb-calendar-heading">{title}</h2>
      <button {...next.buttonProps} ref={nextRef} className="asb-calendar-nav"><ChevronRightIcon /></button>
    </div>
    <table {...gridProps} className="asb-calendar-grid">
      <thead {...headerProps}>
        <tr>{weekDays.map((day, index) => <th key={index} scope="col" className="asb-calendar-weekday">{day}</th>)}</tr>
      </thead>
      <tbody ref={rowsRef}>
        {Array.from({ length: weeksInMonth }, (_, week) => <tr key={week}>
          {state.getDatesInWeek(week).map((date, index) => date
            ? <DayCell key={index} date={date} month={month} state={state} />
            : <td key={index} />)}
        </tr>)}
      </tbody>
    </table>
  </div>;
}

function DayCell({ date, month, state }: {
  date: CalendarDate;
  month: CalendarDate;
  state: ReturnType<typeof useCalendarState>;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const isOutsideMonth = !isSameMonth(month, date);
  const { cellProps, buttonProps, formattedDate, isSelected, isDisabled, isUnavailable, isFocused } =
    useCalendarCell({ date, isOutsideMonth }, state, ref);
  const { focusProps, isFocusVisible } = useFocusRing();
  return <td {...cellProps}>
    <div {...mergeProps(buttonProps, focusProps)} ref={ref} className="asb-calendar-cell"
      data-selected={isSelected || undefined} data-today={isToday(date, state.timeZone) || undefined}
      data-focus-visible={(isFocusVisible && isFocused) || undefined}
      data-outside-month={isOutsideMonth || undefined}
      data-disabled={isDisabled || undefined} data-unavailable={isUnavailable || undefined}>
      {formattedDate}
    </div>
  </td>;
}
