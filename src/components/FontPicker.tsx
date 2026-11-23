import { useEffect, useMemo, useRef, useState } from "react";
import { listSystemFonts } from "../api/client";
import { useI18n } from "../i18n";
import { useMessageState } from "../i18n/use-message-state";
import {
  interfaceFontFamily,
  quotedFontFamily,
  WENQUANYI_ZEN_HEI,
  WENQUANYI_ZEN_HEI_ALIASES,
} from "../lib/font-family";
import { Input } from "./Input";
import { Button } from "./Button";
import { CheckIcon, ChevronDownIcon, SearchIcon } from "./icons";

/** Web fonts bundled with the app: always offered and renderable, even when
 * the system list omits them or enumeration fails. */
const BUNDLED_FONTS = [WENQUANYI_ZEN_HEI, "Noto Sans SC"];

/** Preview stack: the candidate family first, then the bundled default for
 * glyphs the candidate lacks (e.g. a Latin-only family showing the sample). */
function previewStack(font: string): string {
  return `${quotedFontFamily(font)}, var(--asb-font-default), system-ui, sans-serif`;
}

interface Props {
  value: string;
  busy: boolean;
  onChange: (font: string) => void;
}

/**
 * Interface-font picker: the trigger and every option render their font name
 * (plus a fixed sample) in the candidate family itself, so browsing the list
 * is the preview. Options are the bundled web fonts plus the installed system
 * families from `list_system_fonts`.
 */
export function FontPicker({ value, busy, onChange }: Props) {
  const { t } = useI18n();
  const selected = interfaceFontFamily(value);
  const fontLabel = (font: string) =>
    font === WENQUANYI_ZEN_HEI ? t("settings.font.wenquanyiZenHei") : font;
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [systemFonts, setSystemFonts] = useState<string[]>([]);
  const [fontError, setFontError] = useMessageState();
  const [fontRequest, setFontRequest] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const options = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setFontError(null);
    listSystemFonts()
      .then((fonts) => {
        if (!cancelled) setSystemFonts(fonts);
      })
      .catch((error) => {
        if (!cancelled) setFontError(error);
      });
    return () => {
      cancelled = true;
    };
  }, [fontRequest]);

  const close = (refocus = true) => {
    setOpen(false);
    setQuery("");
    if (refocus) trigger.current?.focus();
  };

  const selectFont = (font: string) => {
    onChange(font);
    close();
  };

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) {
        close();
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const current = options.current?.querySelector<HTMLButtonElement>(
      'button[aria-selected="true"]',
    );
    // jsdom has no scrollIntoView; the visual nicety is optional.
    if (typeof current?.scrollIntoView === "function") {
      current.scrollIntoView({ block: "nearest" });
    }
  }, [open]);

  const fonts = useMemo(() => {
    const seen = new Set<string>();
    const merged = [...BUNDLED_FONTS, ...systemFonts].map(interfaceFontFamily).filter((font) => {
      const key = font.toLocaleLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return merged;
    return merged.filter((font) => {
      if (font.toLocaleLowerCase().includes(needle)) return true;
      return font === WENQUANYI_ZEN_HEI && WENQUANYI_ZEN_HEI_ALIASES.some((name) => name.includes(needle));
    });
  }, [systemFonts, query]);

  const onMenuKeyDown = (event: React.KeyboardEvent) => {
    const buttons = Array.from(
      options.current?.querySelectorAll<HTMLButtonElement>('button[role="option"]') ?? [],
    );
    const active = buttons.findIndex((button) => button === document.activeElement);
    const delta = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (delta !== 0 || event.key === "Home" || event.key === "End") {
      event.preventDefault();
      let next: number;
      if (event.key === "Home") next = 0;
      else if (event.key === "End") next = buttons.length - 1;
      else next = active + delta;
      if (next >= 0 && next < buttons.length) buttons[next].focus();
    }
  };

  return (
    <div
      className="asb-font-picker"
      ref={root}
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.preventDefault();
          close();
        }
      }}
    >
      <Button
        variant="unstyled"
        ref={trigger}
        className="asb-select-trigger asb-font-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t("settings.font.pickerAria")}
        disabled={busy}
        onClick={() => (open ? close(false) : setOpen(true))}
      >
        <span className="asb-font-name" style={{ fontFamily: previewStack(selected) }}>
          {fontLabel(selected)}
        </span>
        <span className="asb-select-chevron" aria-hidden="true">
          <ChevronDownIcon />
        </span>
      </Button>

      {open ? (
        <div className="asb-font-menu" onKeyDown={onMenuKeyDown}>
          {fontError && <div className="asb-field" role="alert">
            <p className="asb-field-error">{t("settings.font.loadFailed", { detail: fontError })}</p>
            <Button variant="secondary" onClick={() => setFontRequest((value) => value + 1)}>{t("settings.retry")}</Button>
          </div>}
          <label className="asb-font-search">
            <SearchIcon />
            <Input
              type="search"
              aria-label={t("settings.font.searchAria")}
              placeholder={t("settings.font.searchPlaceholder")}
              value={query}
              autoFocus
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <div
            className="asb-font-options"
            ref={options}
            role="listbox"
            aria-label={t("settings.font.listAria")}
          >
            {fonts.length > 0 ? (
              fonts.map((font) => (
                <Button
                  variant="unstyled"
                  role="option"
                  aria-selected={font === selected}
                  className="asb-font-option"
                  key={font}
                  onClick={() => selectFont(font)}
                >
                  <span className="asb-font-option-name" style={{ fontFamily: previewStack(font) }}>
                    {fontLabel(font)}
                  </span>
                  <span className="asb-font-option-sample" style={{ fontFamily: previewStack(font) }}>
                    中文 Aa 012
                  </span>
                  <span className="asb-font-check" aria-hidden="true">
                    {font === selected && <CheckIcon />}
                  </span>
                </Button>
              ))
            ) : (
              <p className="asb-font-empty">{t("settings.font.empty")}</p>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
