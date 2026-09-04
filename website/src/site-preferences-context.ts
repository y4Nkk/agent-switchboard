import { createContext } from "react";

import type { Locale, SiteContent } from "./content/site-content";

export type SiteTheme = "light" | "dark";

export interface SitePreferencesValue {
  content: SiteContent;
  locale: Locale;
  setLocale: (locale: Locale) => void;
  theme: SiteTheme;
  toggleTheme: () => void;
}

export const SitePreferencesContext = createContext<SitePreferencesValue | null>(null);
