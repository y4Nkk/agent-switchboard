import { useEffect, useMemo, useState, type PropsWithChildren } from "react";

import { siteContentByLocale, type Locale } from "./content/site-content";
import {
  SitePreferencesContext,
  type SitePreferencesValue,
  type SiteTheme,
} from "./site-preferences-context";

export function SitePreferencesProvider({ children }: PropsWithChildren) {
  const [locale, setLocale] = useState<Locale>("zh-CN");
  const [theme, setTheme] = useState<SiteTheme>("light");

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    const content = siteContentByLocale[locale];
    document.documentElement.lang = locale;
    document.title = content.document.title;
    document.querySelector('meta[name="description"]')?.setAttribute(
      "content",
      content.document.description,
    );
  }, [locale]);

  const value = useMemo<SitePreferencesValue>(
    () => ({
      content: siteContentByLocale[locale],
      locale,
      setLocale,
      theme,
      toggleTheme: () => setTheme((current) => (current === "light" ? "dark" : "light")),
    }),
    [locale, theme],
  );

  return <SitePreferencesContext.Provider value={value}>{children}</SitePreferencesContext.Provider>;
}
