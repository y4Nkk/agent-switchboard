import { useEffect, useMemo, useRef, useState } from "react";
import type { ProviderModel } from "../api/client";

/** Group label for models the endpoint did not attribute to a vendor. */
const OTHER_GROUP = "其他";

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
    <span className="asb-model-check" aria-hidden="true">
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

interface Group {
  vendor: string;
  models: ProviderModel[];
}

/** Vendor-grouped models sorted for browsing; `needle` matches either the
 * model id or its vendor, case-insensitively. */
function groupModels(models: ProviderModel[], needle: string): Group[] {
  const groups = new Map<string, ProviderModel[]>();
  for (const model of models) {
    const vendor = model.ownedBy ?? OTHER_GROUP;
    if (!groups.has(vendor)) groups.set(vendor, []);
    groups.get(vendor)!.push(model);
  }
  return [...groups.entries()]
    .map(([vendor, vendorModels]) => ({
      vendor,
      models: vendorModels
        .filter(
          (model) =>
            model.id.toLocaleLowerCase().includes(needle) ||
            vendor.toLocaleLowerCase().includes(needle),
        )
        .sort((a, b) => a.id.localeCompare(b.id)),
    }))
    .filter((group) => group.models.length > 0)
    // The unattributed fallback group stays last regardless of locale.
    .sort((a, b) =>
      a.vendor === OTHER_GROUP ? 1 : b.vendor === OTHER_GROUP ? -1 : a.vendor.localeCompare(b.vendor),
    );
}

interface Props {
  models: ProviderModel[];
  /** Current model id of the field this picker feeds, if any. */
  current: string | null;
  /** Accessible name of the trigger and the search field. */
  ariaLabel: string;
  disabled?: boolean;
  onSelect: (id: string) => void;
}

/**
 * Quick model picker modeled on the CC Switch dropdown: a field-shaped icon
 * trigger opens a searchable menu whose models are grouped by vendor, so a
 * fetched list stays browsable at aggregate-relay scale. Interaction contract
 * (keyboard, close behavior, material) matches the FontPicker listbox.
 */
export function ModelPicker({ models, current, ariaLabel, disabled = false, onSelect }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const options = useRef<HTMLDivElement>(null);

  const close = (refocus = true) => {
    setOpen(false);
    setQuery("");
    if (refocus) trigger.current?.focus();
  };

  const selectModel = (id: string) => {
    onSelect(id);
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

  const needle = query.trim().toLocaleLowerCase();
  const groups = useMemo(() => groupModels(models, needle), [models, needle]);

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
      className="asb-model-picker"
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
        className="asb-model-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => (open ? close(false) : setOpen(true))}
      >
        <ChevronIcon />
      </button>

      {open ? (
        <div className="asb-model-menu" onKeyDown={onMenuKeyDown}>
          <label className="asb-model-search">
            <SearchIcon />
            <input
              type="search"
              aria-label={`搜索${ariaLabel}`}
              placeholder="搜索模型"
              value={query}
              autoFocus
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <div
            className="asb-model-options"
            ref={options}
            role="listbox"
            aria-label={ariaLabel}
          >
            {groups.length > 0 ? (
              groups.map((group) => (
                <div key={group.vendor} role="group" aria-label={group.vendor}>
                  <p className="asb-model-group">{group.vendor}</p>
                  {group.models.map((model) => (
                    <button
                      type="button"
                      role="option"
                      aria-selected={model.id === current}
                      className="asb-model-option"
                      key={model.id}
                      onClick={() => selectModel(model.id)}
                    >
                      <span className="asb-model-option-name">{model.id}</span>
                      {model.id === current ? (
                        <CheckIcon />
                      ) : (
                        <span className="asb-model-check" aria-hidden="true" />
                      )}
                    </button>
                  ))}
                </div>
              ))
            ) : (
              <p className="asb-model-empty">没有找到相关模型</p>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
