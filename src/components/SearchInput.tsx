import { useRef } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Input } from "./Input";
import { Tooltip } from "./Tooltip";
import { CloseIcon, SearchIcon } from "./icons";

interface Props {
  value: string;
  onChange: (value: string) => void;
  ariaLabel: string;
  placeholder?: string;
  disabled?: boolean;
}

export function SearchInput({ value, onChange, ariaLabel, placeholder, disabled }: Props) {
  const { t } = useI18n();
  const input = useRef<HTMLInputElement>(null);
  const clear = () => {
    onChange("");
    input.current?.focus();
  };
  return (
    <div className="asb-search-input" role="search">
      <span className="asb-search-input-icon" aria-hidden="true"><SearchIcon /></span>
      <Input ref={input} type="search" value={value} aria-label={ariaLabel}
        placeholder={placeholder} disabled={disabled} autoComplete="off"
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && value) {
            event.preventDefault();
            event.stopPropagation();
            clear();
          }
        }} />
      {value.length > 0 && <Tooltip label={t("common.clearSearch")}>
        <Button variant="icon" size="compact" disabled={disabled}
          aria-label={t("common.clearSearch")} onClick={clear}><CloseIcon /></Button>
      </Tooltip>}
    </div>
  );
}
