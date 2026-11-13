import type { Dispatch, SetStateAction } from "react";
import { useI18n } from "../../i18n";
import { claudeOptions } from "../../lib/provider-drafts";
import { Textarea } from "../Textarea";
import type { ProviderEditorDraft } from "./draft";

/** Raw text belongs to this editor session; blank lines must survive typing. */
export function AvailableModelsField({ busy, setDraft, text, onTextChange }: {
  busy: boolean; setDraft: Dispatch<SetStateAction<ProviderEditorDraft>>; text: string; onTextChange: (value: string) => void;
}) {
  const { t } = useI18n();
  return <label className="asb-field"><span>{t("providers.editor.availableModels")}</span>
    <Textarea code rows={3} value={text} disabled={busy} onChange={(event) => {
      const value = event.target.value;
      onTextChange(value);
      const models = value.split("\n").map((id) => id.trim()).filter(Boolean);
      setDraft((current) => ({ ...current,
        modelOptions: claudeOptions(current.modelOptions, { availableModels: models.length ? models : null }),
      }));
    }} />
  </label>;
}
