import type { CodexTransportClass, CommandError } from "../api/shared";
import type { LocalizedMessage } from "../api/switching";
import type { TFunction } from "./context";
import { messages, type MessageKey, type TemplateArgs } from "./messages.ts";

/** Catalog keys of the shared transport-failure classes; the backend owns
 * the classification, this catalog owns the text. */
export const TRANSPORT_CLASS_KEYS: Record<CodexTransportClass, MessageKey> = {
  dns: "errors.transport.dns",
  timeout: "errors.transport.timeout",
  connect: "errors.transport.connect",
  tls: "errors.transport.tls",
  other: "errors.transport.other",
};

/** App-owned messages remain structured through state and async callbacks;
 * the params are checked against the key's template placeholders. */
export function uiMessage<K extends MessageKey>(key: K, ...args: TemplateArgs<K>): CommandError {
  return { code: "ui-message", message: "", messageKey: key, params: args[0] };
}

export function errorText(error: unknown, t: TFunction): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    if ("code" in error && "message" in error) return commandErrorText(error as CommandError, t);
    if ("message" in error && typeof error.message === "string") return error.message;
  }
  return t("common.operationFailed");
}

/** Keep the backend diagnostic when the localized explanation omits it. */
export function errorDiagnosticText(error: unknown, t: TFunction): string | null {
  if (!error || typeof error !== "object" || !("message" in error)
    || typeof error.message !== "string" || !error.message.trim()) return null;
  return errorText(error, t).includes(error.message) ? null : error.message;
}

/** Renders one backend warning in the current language. Unknown keys fall
 * back to the scrubbed `text` so a message can never go blank. */
export function localizedMessageText(message: LocalizedMessage, t: TFunction): string {
  return commandErrorText({
    code: "localized-message", message: message.text,
    messageKey: message.key, params: message.params,
  }, t);
}

/** Resolves a backend catalog string (spec labels, groups, directory
 * titles) that may be a catalog key. Known keys render in the current
 * language; external diagnostics and user-provided values render verbatim. */
export function catalogText(text: string, t: TFunction): string {
  return text in messages ? t(text as MessageKey) : text;
}

/**
 * Renders a command error in the current language. App-owned failures carry
 * `messageKey` (+ `params`); their scrubbed `message` stays available on the
 * error object as diagnostic detail. Unknown keys fall back to the raw
 * message, so an unmatched key can never blank an error.
 *
 * Convention: a parameter whose name ends in `Key` carries a catalog key
 * itself (e.g. `stageKey: "errors.switch.stage.backup"`) and is rendered
 * through the catalog; unknown nested keys render their raw value. The key
 * and its params are Rust-owned runtime data — the one crossing where the
 * renderer cannot statically check placeholder names against the template —
 * so the final interpolation is the single deliberately unchecked call.
 */
export function commandErrorText(error: CommandError, t: TFunction): string {
  if (error.messageKey && error.messageKey in messages) {
    const key = error.messageKey as MessageKey;
    const raw = error.params;
    if (!raw || typeof raw !== "object") return t(key);
    const params: Record<string, string | number> = {};
    for (const [name, value] of Object.entries(raw as Record<string, unknown>)) {
      if (value === null || value === undefined) continue;
      params[name] =
        name.endsWith("Key") && typeof value === "string" && value in messages
          ? t(value as MessageKey)
          : (value as string | number);
    }
    return t(key, params as never);
  }
  return error.message;
}
