import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

import { actionProps } from "../content/site-content";
import { useSitePreferences } from "../use-site-preferences";
import {
  CheckIcon,
  ChevronDownIcon,
  GithubIcon,
  GlobeIcon,
  MoonIcon,
  SunIcon,
} from "./icons";

function LocalePicker() {
  const { content, locale, setLocale } = useSitePreferences();
  const [open, setOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const menuId = useId();
  const activeLocale = content.header.locales.find((item) => item.id === locale);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (!popoverRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, []);

  const focusOption = (index: number) => {
    optionRefs.current[index]?.focus();
  };

  const openAt = (index: number) => {
    setOpen(true);
    window.requestAnimationFrame(() => focusOption(index));
  };

  const closeAndReturnFocus = () => {
    setOpen(false);
    triggerRef.current?.focus();
  };

  const selectLocale = (nextLocale: typeof locale) => {
    setLocale(nextLocale);
    closeAndReturnFocus();
  };

  const onTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const activeIndex = content.header.locales.findIndex((item) => item.id === locale);
    if (event.key === "ArrowDown" || event.key === "Home") {
      event.preventDefault();
      openAt(0);
    } else if (event.key === "ArrowUp" || event.key === "End") {
      event.preventDefault();
      openAt(content.header.locales.length - 1);
    } else if (event.key === "Escape" && open) {
      event.preventDefault();
      closeAndReturnFocus();
    } else if (event.key === " ") {
      event.preventDefault();
      openAt(activeIndex);
    }
  };

  const onOptionKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const lastIndex = content.header.locales.length - 1;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusOption((index + 1) % content.header.locales.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusOption((index - 1 + content.header.locales.length) % content.header.locales.length);
    } else if (event.key === "Home") {
      event.preventDefault();
      focusOption(0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusOption(lastIndex);
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeAndReturnFocus();
    }
  };

  return (
    <div className="locale-picker" ref={popoverRef}>
      <button
        type="button"
        ref={triggerRef}
        className="header-control locale-trigger"
        aria-label={content.header.localeMenuLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={menuId}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={onTriggerKeyDown}
      >
        <GlobeIcon />
        <span>{activeLocale?.short}</span>
        <ChevronDownIcon />
      </button>
      {open ? (
        <div id={menuId} className="locale-menu" role="menu" aria-label={content.header.localeMenuLabel}>
          {content.header.locales.map((item, index) => (
            <button
              key={item.id}
              ref={(element) => {
                optionRefs.current[index] = element;
              }}
              type="button"
              role="menuitemradio"
              aria-checked={item.id === locale}
              className="locale-option"
              onClick={() => {
                selectLocale(item.id);
              }}
              onKeyDown={(event) => onOptionKeyDown(event, index)}
            >
              <span className="locale-option-short">{item.short}</span>
              <span>{item.label}</span>
              {item.id === locale ? <CheckIcon /> : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function HeaderControls() {
  const { content, theme, toggleTheme } = useSitePreferences();
  const ThemeIcon = theme === "light" ? MoonIcon : SunIcon;
  const themeLabel = theme === "light" ? content.header.themeToDarkLabel : content.header.themeToLightLabel;

  return (
    <div className="site-header-controls">
      <LocalePicker />
      <button
        type="button"
        className="header-control header-icon-control"
        aria-label={themeLabel}
        aria-pressed={theme === "dark"}
        onClick={toggleTheme}
      >
        <ThemeIcon />
      </button>
      <a
        className="header-control header-icon-control"
        aria-label={content.header.githubLabel}
        {...actionProps({ href: content.repoUrl, external: true })}
      >
        <GithubIcon size={19} />
      </a>
    </div>
  );
}

export function SiteHeader() {
  const { content } = useSitePreferences();
  return (
    <header className="site-header">
      <div className="site-container site-header-inner">
        <a className="site-brand" href="/">
          <img src="/favicon.svg" alt="" />
          <span className="site-brand-label">{content.brand}</span>
        </a>
        <nav className="site-nav" aria-label={content.header.navigationLabel}>
          {content.nav.map((item) => (
            <a key={item.href} className="site-nav-link" {...actionProps(item)}>
              {item.label}
            </a>
          ))}
        </nav>
        <HeaderControls />
      </div>
    </header>
  );
}
