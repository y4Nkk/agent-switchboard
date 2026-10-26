import { useState, type Dispatch, type SetStateAction } from "react";
import { useI18n } from "../../i18n";
import { claudeOptions } from "../../lib/provider-drafts";
import { Textarea } from "../Textarea";
import type { ProviderEditorDraft } from "./draft";

/** Raw text belongs to this editor session; blank lines must survive typing. */
export function AvailableModelsField({ draft, busy, setDraft }: {
  draft: ProviderEditorDraft; busy: boolean; setDraft: Dispatch<SetStateAction<ProviderEditorDraft>>;
}) {
  const { t } = useI18n();
  const [text, setText] = useState(() => draft.modelOptions?.kind === "claude"
    ? (draft.modelOptions.availableModels ?? []).join("\n") : "");
  return <label className="asb-field"><span>{t("providers.editor.availableModels")}</span>
    <Textarea code rows={3} value={text} disabled={busy} onChange={(event) => {
      const value = event.target.value;
      setText(value);
      const models = value.split("\n").map((id) => id.trim()).filter(Boolean);
      setDraft((current) => ({ ...current,
        modelOptions: claudeOptions(current.modelOptions, { availableModels: models.length ? models : null }),
      }));
    }} />
  </label>;
}
