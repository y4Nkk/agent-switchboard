/*
 * Typed client boundary. This module is the ONLY frontend file that talks to
 * the Tauri backend; components consume these typed wrappers and never build
 * configuration text or filesystem paths themselves.
 * (Enforced by boundary.test.ts.)
 */
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { isBrowserDevelopment } from "../lib/runtime";
import {
  WEB_DEVELOPMENT_BACKEND_HEALTH_URL,
  WEB_DEVELOPMENT_BACKEND_ORIGIN,
  WEB_DEVELOPMENT_BACKEND_READY_INTERVAL_MS,
} from "../dev/web-backend";
import type { AppKind, CommandError } from "./shared";
import type { AppSettings } from "./settings";
import type { CodexOfficialQuota, UsageSnapshot } from "./usage";

type InvokeArgs = Record<string, unknown>;

export type TrayUsage =
  | { kind: "script"; reading: UsageSnapshot }
  | { kind: "official"; reading: CodexOfficialQuota };

export interface TraySnapshot {
  providers: Array<{
    id: string;
    app: AppKind;
    name: string;
    active: boolean;
    usage: TrayUsage | null;
  }>;
  settings: AppSettings | null;
  error: string | null;
  switching: boolean;
}

export const getTraySnapshot = (): Promise<TraySnapshot> => invoke("tray_snapshot");
export const trayReady = (): Promise<void> => invoke("tray_ready");
export const hideTray = (): Promise<void> => invoke("tray_hide");
export const openTrayMain = (): Promise<void> => invoke("tray_open_main");
export const switchTrayProvider = (profileId: string): Promise<void> => invoke("tray_switch", { profileId });
export const quitTray = (): Promise<void> => invoke("tray_quit");
/** Report full content height; native configuration and work area own the final size. */
export const resizeTray = (height: number): Promise<void> => invoke("tray_resize", { height });
export function onTrayChanged(handler: () => void): Promise<() => void> {
  if (isBrowserDevelopment) {
    // The browser development bridge has no Tauri event transport. Poll
    // cache consumers only; upstream refresh remains owned by the backend.
    const timer = window.setInterval(handler, 5_000);
    return Promise.resolve(() => window.clearInterval(timer));
  }
  return listen("tray-changed", handler);
}
/** Emits whenever either real client configuration file changes on disk. */
export function onClientConfigChanged(handler: () => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return listen("client-config-changed", handler);
}
/** Probe lifecycle invalidation; results always come from the typed read API.
 * Browser development uses command refreshes and the same live-state polling. */
export function onCodexProbeChanged(handler: (batchId: string) => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return listen<string>("codex-probe-changed", (event) => handler(event.payload));
}
export function onTrayError(handler: (error: CommandError) => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return listen<CommandError>("tray-error", (event) => handler(event.payload));
}
/** Native blocked exits reveal the exact workspace that can finish recovery. */
export function onExitRecovery(handler: (destination: "radar" | "backups" | "clientManagement") => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return listen<"radar" | "backups" | "clientManagement">("exit-recovery", (event) => handler(event.payload));
}
export function onDesktopSettingsError(handler: (error: CommandError) => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return listen<CommandError>("desktop-settings-error", (event) => handler(event.payload));
}

interface WebCommandResponse<T> {
  kind: "success" | "failure";
  result?: T;
  error?: CommandError;
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

async function waitForWebBackend(): Promise<void> {
  for (;;) {
    try {
      const response = await fetch(WEB_DEVELOPMENT_BACKEND_HEALTH_URL, { cache: "no-store" });
      if (response.status === 204) return;
    } catch {
      // The browser-development backend starts after Vite; wait for its health endpoint.
    }
    await delay(WEB_DEVELOPMENT_BACKEND_READY_INTERVAL_MS);
  }
}

/** In browser development, calls the loopback Tauri helper only after it is ready.
 * Desktop and test code keep the native Tauri invoke transport. */
export async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  if (!isBrowserDevelopment) {
    return args === undefined ? tauriInvoke<T>(command) : tauriInvoke<T>(command, args);
  }

  await waitForWebBackend();
  const response = await fetch(`${WEB_DEVELOPMENT_BACKEND_ORIGIN}/invoke`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command, args }),
  }).catch(() => {
    throw {
      code: "web-backend-unavailable",
      message: "本机开发后端未就绪；请通过 npm run dev 启动应用",
      messageKey: "errors.dev.backendNotReady",
    } satisfies CommandError;
  });

  const payload = (await response.json().catch(() => null)) as WebCommandResponse<T> | null;
  if (!response.ok || !payload) {
    throw {
      code: "web-backend-unavailable",
      message: "本机开发后端没有返回有效响应",
      messageKey: "errors.dev.noResponse",
    } satisfies CommandError;
  }
  if (payload.kind === "failure") {
    throw payload.error;
  }
  return payload.result as T;
}

/* Domain modules own the typed command families; this boundary re-exports
 * their public surface so consumers keep importing from "./api/client". */
export * from "./shared";
export * from "./updates";
export * from "./usage";
export * from "./codex-probe";
export * from "./model-usage";
export * from "./request-usage";
export * from "./resource-usage";
export * from "./providers";
export * from "./provider-request";
export * from "./switching";
export * from "./status";
export * from "./settings";
export * from "./subagent-settings";
export * from "./sessions";
export * from "./session-search";
export * from "./discovery";
export * from "./official-login";
export * from "./extensions/types";
export * from "./extensions/commands";

/* Integrated title bar window controls (undecorated window). These invoke
   app-owned commands in src-tauri (window_minimize / window_toggle_maximize /
   window_close / window_is_maximized), keeping all backend access inside this
   boundary. Maximize state is re-synced through window resize events. */
export function minimizeWindow(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("window_minimize");
}

export function toggleMaximizeWindow(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("window_toggle_maximize");
}

export function closeWindow(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("window_close");
}

/** Restarts the native desktop process so creation-time WebView settings apply. */
export function restartApplication(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("restart_application");
}

/** Opens the native directory picker and returns the picked absolute path;
 * `null` means the user closed it without choosing. The dialog itself lives
 * in the backend, so browser development gets the same native picker. */
export function pickDirectory(): Promise<string | null> {
  return invoke<string | null>("pick_directory");
}

/** Opens the native file picker limited to `extensions` and returns the
 * selected absolute path; `null` means the user closed it without choosing. */
export function pickFile(filterName: string, extensions: string[]): Promise<string | null> {
  return invoke<string | null>("pick_file", { filterName, extensions });
}

export function getWindowMaximized(): Promise<boolean> {
  if (isBrowserDevelopment) return Promise.resolve(false);
  return invoke<boolean>("window_is_maximized");
}

export function onWindowResized(handler: () => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return getCurrentWindow().onResized(() => handler());
}

/** Native visibility includes close-to-tray and minimization, independently of focus. */
export async function onWorkspaceVisibilityChanged(handler: (visible: boolean) => void): Promise<() => void> {
  if (isBrowserDevelopment) { handler(true); return () => {}; }
  let revision = 0;
  const unlisten = await listen<boolean>("workspace-visibility-changed", (event) => {
    revision += 1;
    handler(event.payload);
  });
  const beforeRead = revision;
  try {
    const visible = await invoke<boolean>("workspace_visible");
    if (revision === beforeRead) handler(visible);
  } catch (error) { console.error("Initial workspace visibility could not be read", error); }
  return unlisten;
}
