import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";

/** Values stay visible; a control appears only for the value the user is changing. */
export function ProviderValue({ label, value, display, options, disabled, required, numeric, min, max, preserveText, onChange }: {
  label: string; value: string; display?: string; options?: { value: string; label: string }[];
  disabled: boolean; required?: boolean; numeric?: boolean; min?: number; max?: number; preserveText?: boolean;
  onChange: (value: string) => void;
}) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [raw, setRaw] = useState(value);
  const input = useRef<HTMLInputElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const choice = Boolean(options);
  useEffect(() => { if (!editing) setRaw(value); }, [value, editing]);
  useEffect(() => {
    if (editing) {
      if (choice) root.current?.querySelector<HTMLButtonElement>('[role="combobox"]')?.focus();
      else input.current?.focus();
    }
  }, [editing, choice]);
  const open = editing || Boolean(required && !value);
  const finish = () => {
    if (input.current && !input.current.reportValidity()) return;
    setEditing(false);
    requestAnimationFrame(() => root.current?.querySelector<HTMLButtonElement>("button")?.focus());
  };
  return <div ref={root} className="asb-provider-value">
    {open ? <>
      {options ? <Select ariaLabel={label} value={value} options={options} disabled={disabled}
        onChange={onChange} /> : <Input ref={input} aria-label={label} value={preserveText && editing ? raw : value} required={required} disabled={disabled}
        type={numeric ? "number" : "text"} min={min} max={max} step={numeric ? 1 : undefined}
        placeholder={t(required ? "providerSetup.unconfirmed" : "providerSetup.defaults")}
        onChange={(event) => { setEditing(true); setRaw(event.target.value); onChange(event.target.value); }} />}
      <Button variant="ghost" size="compact" disabled={disabled || Boolean(required && !value)}
        onClick={finish}>{t("providerSetup.done")}</Button>
    </> : <>
      <span>{display ?? options?.find((option) => option.value === value)?.label ?? (value || t("providerSetup.unconfirmed"))}</span>
      <Button variant="ghost" size="compact" disabled={disabled} aria-label={t("providerSetup.editValue", { label })}
        onClick={() => { setRaw(value); setEditing(true); }}>{t("providerSetup.edit")}</Button>
    </>}
  </div>;
}
