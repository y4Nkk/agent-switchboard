import { useMemo, useSyncExternalStore } from "react";
import { currentLanguage, subscribeLanguage } from "./current";
import { entryLanguage, formatMessage } from "./format";
import { messages, type MessageKey, type TemplateArgs } from "./messages";
import type { ResolvedLanguage } from "./types";

/** The template owns its param contract: a call whose params do not match the
 * key's `{placeholders}` fails to compile (see `TemplateArgs`). */
export type TFunction = <K extends MessageKey>(key: K, ...args: TemplateArgs<K>) => string;
export interface I18n {
  language: ResolvedLanguage;
  t: TFunction;
}

/** Components and plain formatters read the same committed language snapshot. */
export function useI18n(): I18n {
  const language = useSyncExternalStore(subscribeLanguage, currentLanguage, currentLanguage);
  return useMemo(() => ({
    language,
    t: <K extends MessageKey>(key: K, ...args: TemplateArgs<K>) =>
      formatMessage(entryLanguage(messages[key], language), args[0]),
  }), [language]);
}
