/**
 * The single message catalog. Every domain file contributes its `messages`
 * and each entry is a [zh-CN, en-US] pair, so both languages are defined
 * together and stay key-identical by construction. `MessageKey` is derived
 * from this object: translating an unknown key is a type error, and each
 * domain owns a reserved key prefix so names cannot collide silently.
 */
import { messages as backup } from "./messages/backup.ts";
import { messages as clientConfig } from "./messages/clientConfig.ts";
import { messages as cmdConfig } from "./messages/cmd-config.ts";
import { messages as cmdExtlib } from "./messages/cmd-extlib.ts";
import { messages as cmdExtops } from "./messages/cmd-extops.ts";
import { messages as cmdMisc } from "./messages/cmd-misc.ts";
import { messages as cmdSwitch } from "./messages/cmd-switch.ts";
import { messages as codex } from "./messages/codex.ts";
import { messages as common } from "./messages/common.ts";
import { messages as errors } from "./messages/errors.ts";
import { messages as extcap } from "./messages/extcap.ts";
import { messages as extensions } from "./messages/extensions.ts";
import { messages as format } from "./messages/format.ts";
import { messages as gateway } from "./messages/gateway.ts";
import { messages as importDiscovery } from "./messages/importDiscovery.ts";
import { messages as mcp } from "./messages/mcp.ts";
import { messages as operations } from "./messages/operations.ts";
import { messages as ownership } from "./messages/ownership.ts";
import { messages as protocols } from "./messages/protocols.ts";
import { messages as providers } from "./messages/providers.ts";
import { messages as probeVisual } from "./messages/probe-visual.ts";
import { messages as probeNotifications } from "./messages/probe-notifications.ts";
import { messages as providerDiagnostics } from "./messages/provider-diagnostics.ts";
import { messages as providerDiagnosticsBackend } from "./messages/provider-diagnostics-backend.ts";
import { messages as sessions } from "./messages/sessions.ts";
import { messages as storage } from "./messages/storage.ts";
import { messages as skillWorkbench } from "./messages/skill-workbench.ts";
import { messages as settings } from "./messages/settings.ts";
import { messages as tray } from "./messages/tray.ts";
import { messages as usage } from "./messages/usage.ts";
import { messages as resources } from "./messages/resources.ts";

export const messages = Object.freeze({
  ...backup,
  ...clientConfig,
  ...cmdConfig,
  ...cmdExtlib,
  ...cmdExtops,
  ...cmdMisc,
  ...cmdSwitch,
  ...codex,
  ...common,
  ...errors,
  ...extcap,
  ...extensions,
  ...format,
  ...gateway,
  ...importDiscovery,
  ...mcp,
  ...operations,
  ...ownership,
  ...protocols,
  ...providers,
  ...probeVisual,
  ...probeNotifications,
  ...providerDiagnostics,
  ...providerDiagnosticsBackend,
  ...sessions,
  ...storage,
  ...settings,
  ...skillWorkbench,
  ...tray,
  ...usage,
  ...resources,
} as const);

export type MessageKey = keyof typeof messages;
export type { MessageEntry, MessageParams, ResolvedLanguage } from "./types";

// The message template is the single owner of its interpolation contract:
// placeholder names are derived from the literal itself and every translator
// call is checked against them, so a renamed `{detail}` can never silently
// mismatch a `detailKey` param again.

/** Placeholder names inside one template literal, e.g. `"年{month}月"` → `"month"`. */
type Placeholders<S extends string> =
  S extends `${string}{${infer Name}}${infer Rest}` ? Name | Placeholders<Rest> : never;

/** The zh template stands for both languages; zh/en parity is asserted below. */
type ZhTemplate<K extends MessageKey> = (typeof messages)[K][0];

/** Value type of one placeholder: `*Key` names carry catalog keys and resolve
 * recursively through the catalog; everything else is a display value. */
type PlaceholderValue<Name extends string> = Name extends `${string}Key` ? MessageKey : string | number;

/** The exact params object one message key accepts. */
export type MessageParamsOf<K extends MessageKey> = {
  [Name in Placeholders<ZhTemplate<K>>]: PlaceholderValue<Name>;
};

/** Translator argument tuple: no placeholders → no params accepted. For a
 * union of keys this distributes per member, so a no-args call stays valid
 * when every key in the union is placeholder-free. */
export type TemplateArgs<K extends MessageKey> = K extends unknown
  ? Placeholders<ZhTemplate<K>> extends never ? [] : [params: MessageParamsOf<K>]
  : never;

/** Fails to compile, naming the key, when a template's zh and en entries
 * interpolate different placeholder names. */
type PlaceholderParityMismatch = {
  [K in MessageKey]: Placeholders<ZhTemplate<K>> extends Placeholders<(typeof messages)[K][1]>
    ? Placeholders<(typeof messages)[K][1]> extends Placeholders<ZhTemplate<K>> ? never : K
    : K;
}[MessageKey];
type ParityAssertion = [PlaceholderParityMismatch] extends [never] ? "ok" : PlaceholderParityMismatch;
export const MESSAGE_PLACEHOLDER_PARITY: ParityAssertion = "ok";
