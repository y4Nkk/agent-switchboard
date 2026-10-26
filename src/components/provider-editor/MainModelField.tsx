import type { Dispatch, SetStateAction } from "react";
import type {
  ClaudeModelSettings,
  ProviderModel,
  LocalizedMessage,
} from "../../api/client";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Input } from "../Input";
import { ModelPicker } from "../ModelPicker";
import { Tooltip } from "../Tooltip";
import { UpdateIcon } from "../icons";
import type { ProviderEditorDraft } from "./draft";
import { claudeOptions } from "../../lib/provider-drafts";

interface Props {
  draft: ProviderEditorDraft;
  busy: boolean;
  baseUrl: string;
  claudeSettings: ClaudeModelSettings | null;
  models: ProviderModel[] | null;
  modelsBusy: boolean;
  modelsError: string | null;
  modelsEndpointError: string | null;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
  fetchModels: () => Promise<unknown> | void;
  setDraft: Dispatch<SetStateAction<ProviderEditorDraft>>;
}

function PrimaryModelInput({ draft, busy, setDraft }: Pick<Props, "draft" | "busy" | "setDraft">) {
  const { t } = useI18n();
  return (
    <Input aria-label={t("providers.editor.mainModel")} value={draft.model ?? ""} disabled={busy} placeholder={t("providers.editor.optional")}
      onChange={(event) => setDraft((current) => {
        const model = event.target.value;
        if (!model.trim() && current.modelOptions?.kind === "claude") {
          return { ...current, model, modelOptions: { ...current.modelOptions, primaryOneM: false } };
        }
        return { ...current, model };
      })} />
  );
}

/** Main model field with the quick picker, Claude context flag, and model fetch. */
export function MainModelField({
  draft,
  busy,
  baseUrl,
  claudeSettings,
  models,
  modelsBusy,
  modelsError,
  modelsEndpointError,
  userConfigModel,
  userConfigWarnings,
  fetchModels,
  setDraft,
}: Props) {
  const { t } = useI18n();
  return (
    <div className="asb-field">
      <span>{t("providers.editor.mainModel")}</span>
      <div className="asb-model-control">
        <PrimaryModelInput draft={draft} busy={busy} setDraft={setDraft} />
        {models && (
          <ModelPicker
            models={models.map(({ id, ownedBy }) => ({ value: id, label: id, group: ownedBy }))}
            current={draft.model}
            ariaLabel={t("providers.editor.pickModel")}
            disabled={busy}
            onSelect={(model) => setDraft((current) => ({ ...current, model }))}
          />
        )}
        <Checkbox
          label="1M"
          ariaLabel={t("providers.editor.primaryOneMAria")}
          checked={claudeSettings?.primaryOneM ?? false}
          disabled={busy || !draft.model?.trim()}
          onChange={(enabled) =>
            setDraft((current) => ({
              ...current,
              modelOptions: claudeOptions(current.modelOptions, { primaryOneM: enabled }),
            }))
          }
        />
        <div className="asb-provider-model-actions">
          <Tooltip label={modelsBusy ? t("providers.editor.fetchingModels") : modelsEndpointError ?? t("providers.request.fetchModels")}>
            <Button
              variant="icon"
              aria-label={modelsBusy ? t("providers.editor.fetchingModels") : modelsEndpointError ?? t("providers.request.fetchModels")}
              aria-busy={modelsBusy || undefined}
              disabled={busy || modelsBusy || !baseUrl || !!modelsEndpointError}
              onClick={() => void fetchModels()}
            >
              <UpdateIcon />
            </Button>
          </Tooltip>
        </div>
      </div>
      {userConfigModel && (
        <p className="asb-scope-note">{t("providers.editor.userConfigModel", { model: userConfigModel })}</p>
      )}
      {userConfigWarnings.map((warning) => (
        <p key={warning.key} className="asb-scope-note asb-warn-text">
          {localizedMessageText(warning, t)}
        </p>
      ))}
      {modelsEndpointError && <span className="asb-warn-text">{modelsEndpointError}</span>}
      {modelsError && modelsError !== modelsEndpointError && <span className="asb-warn-text">{modelsError}</span>}
    </div>
  );
}
