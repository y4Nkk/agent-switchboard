import type { ResponsesOptions } from "../../api/client";
import { useI18n } from "../../i18n";
import { Select } from "../Select";

interface Props {
  options: ResponsesOptions | null;
  busy: boolean;
  onChange: (next: ResponsesOptions) => void;
}

export function ResponsesOptionsFields({ options, busy, onChange }: Props) {
  const { t } = useI18n();
  const minimal = options?.requestMode === "minimal";
  return (
    <section className="asb-provider-advanced-group" aria-label={t("providers.editor.responses.title")}>
      <h4 className="asb-provider-advanced-heading">
        <span>{t("providers.editor.responses.title")}</span>
        <span className="asb-provider-advanced-value">
          {!options ? t("providers.editor.responses.incomplete") : t("providers.editor.responses.summary", {
            mode: minimal ? t("providers.editor.responses.minimal") : t("providers.editor.responses.standard"),
          })}
        </span>
      </h4>
      {!options ? <p role="alert" className="asb-scope-note asb-warn-text">{t("providers.editor.responses.missingNote")}</p>
        : <div className="asb-provider-field-grid">
          <div className="asb-field">
            <span>{t("providers.editor.responses.requestMode")}</span>
            <Select ariaLabel={t("providers.editor.responses.requestMode")} value={options.requestMode} disabled={busy}
              options={[{ value: "standard", label: t("providers.editor.responses.standard") }, { value: "minimal", label: t("providers.editor.responses.minimal") }]}
              onChange={(value) => {
                const requestMode = value as ResponsesOptions["requestMode"];
                onChange({ requestMode });
              }} />
            <p className="asb-scope-note">
              {minimal
                ? t("providers.editor.responses.minimalNote")
                : t("providers.editor.responses.standardNote")}
            </p>
          </div>
          <p className="asb-scope-note">{t("providers.editor.responses.transportNote")}</p>
        </div>}
    </section>
  );
}
