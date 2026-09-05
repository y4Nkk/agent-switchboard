import { afterEach, expect, it } from "vitest";
import type { AppSettings } from "../api/client";
import { applyAppAppearance } from "./app-appearance";

afterEach(() => applyAppAppearance(null));

it("shares explicit appearance and clears overrides for system preferences", () => {
  const settings: AppSettings = {
    closeBehavior: "hideToTray", theme: "dark", motion: "reduce", alwaysOnTop: false,
    launchAtLogin: false, hardwareAcceleration: true, interfaceFont: "Example Font",
    runtimeLogLevel: "info", collapsedUsageIds: [],
  };
  applyAppAppearance(settings);
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(document.documentElement.dataset.motion).toBe("reduce");
  expect(document.documentElement.style.getPropertyValue("--asb-font-user")).toContain("Example Font");
  applyAppAppearance({ ...settings, theme: "system", motion: "system" });
  expect(document.documentElement.dataset.theme).toBeUndefined();
  expect(document.documentElement.dataset.motion).toBeUndefined();
  applyAppAppearance(null);
  expect(document.documentElement.style.getPropertyValue("--asb-font-user")).toBe("");
});
