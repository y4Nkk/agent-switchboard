import type { AppSettings } from "../api/client";
import { quotedFontFamily } from "./font-family";

/** Shared appearance contract for the main window and tray webview. */
export function applyAppAppearance(settings: AppSettings | null): void {
  const root = document.documentElement;
  if (!settings || settings.theme === "system") delete root.dataset.theme;
  else root.dataset.theme = settings.theme;
  if (!settings || settings.motion === "system") delete root.dataset.motion;
  else root.dataset.motion = settings.motion;
  if (settings) root.style.setProperty("--asb-font-user", quotedFontFamily(settings.interfaceFont));
  else root.style.removeProperty("--asb-font-user");
}
