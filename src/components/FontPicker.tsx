import { useEffect, useMemo, useRef, useState } from "react";
import { listSystemFonts } from "../api/client";
import { quotedFontFamily } from "../lib/font-family";

/** Web fonts bundled with the app: always offered and renderable, even when
 * the system list omits them or enumeration fails. */
const BUNDLED_FONTS = ["Noto Sans SC"];

/** Preview stack: the candidate family first, then the bundled default for
 * glyphs the candidate lacks (e.g. a Latin-only family showing the sample). */
function previewStack(font: string): string {
  return `${quotedFontFamily(font)}, ${quotedFontFamily(BUNDLED_FONTS[0])}, system-ui, sans-serif`;
}

function ChevronIcon() {
  return (
    <span className="asb-select-chevron" aria-hidden="true">
      <svg viewBox="0 0 16 16" fill="none">
        <path
          d="M3.5 6 L8 10.5 L12.5 6"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}

function SearchIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.4" stroke="currentColor" strokeWidth="1.6" />
      <path d="M10.4 10.4 L13.5 13.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <span className="asb-font-check" aria-hidden="true">
      <svg viewBox="0 0 12 12" fill="none">
        <polyline
          points="2.4 6.4 5 9 9.6 3.4"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
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
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [systemFonts, setSystemFonts] = useState<string[]>([]);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const options = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    listSystemFonts()
      .then((fonts) => {
        if (!cancelled) setSystemFonts(fonts);
      })
      .catch(() => {
        // Enumeration failed: the bundled default stays selectable.
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
    const merged = [...BUNDLED_FONTS, ...systemFonts].filter((font) => {
      const key = font.toLocaleLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    const needle = query.trim().toLocaleLowerCase();
    return needle
      ? merged.filter((font) => font.toLocaleLowerCase().includes(needle))
      : merged;
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
      <button
        type="button"
        ref={trigger}
        className="asb-select-trigger asb-font-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label="选择界面字体"
        disabled={busy}
        onClick={() => (open ? close(false) : setOpen(true))}
      >
        <span className="asb-font-name" style={{ fontFamily: previewStack(value) }}>
          {value}
        </span>
        <ChevronIcon />
      </button>

      {open ? (
        <div className="asb-font-menu" onKeyDown={onMenuKeyDown}>
          <label className="asb-font-search">
            <SearchIcon />
            <input
              type="search"
              aria-label="搜索字体"
              placeholder="搜索字体"
              value={query}
              autoFocus
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <div
            className="asb-font-options"
            ref={options}
            role="listbox"
            aria-label="可选字体"
          >
            {fonts.length > 0 ? (
              fonts.map((font) => (
                <button
                  type="button"
                  role="option"
                  aria-selected={font === value}
                  className="asb-font-option"
                  key={font}
                  onClick={() => selectFont(font)}
                >
                  <span className="asb-font-option-name" style={{ fontFamily: previewStack(font) }}>
                    {font}
                  </span>
                  <span className="asb-font-option-sample" style={{ fontFamily: previewStack(font) }}>
                    中文 Aa 012
                  </span>
                  {font === value ? <CheckIcon /> : <span className="asb-font-check" aria-hidden="true" />}
                </button>
              ))
            ) : (
              <p className="asb-font-empty">没有找到相关字体</p>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
