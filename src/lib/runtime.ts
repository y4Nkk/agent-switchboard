/**
 * The browser-facing development page is backed by the same local Tauri
 * process as the desktop shell. Tests use mocked Tauri APIs, so they retain
 * the desktop client path.
 */
export const isBrowserDevelopment =
  import.meta.env.DEV &&
  import.meta.env.MODE !== "test" &&
  typeof window !== "undefined" &&
  !("__TAURI_INTERNALS__" in window);
