import { useContext } from "react";

import { SitePreferencesContext } from "./site-preferences-context";

export function useSitePreferences() {
  const value = useContext(SitePreferencesContext);
  if (!value) throw new Error("useSitePreferences must be used within SitePreferencesProvider");
  return value;
}
