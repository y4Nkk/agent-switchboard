import type { AppKind } from "../../api/client";
import { providerCatalog, providerPresets, type ProviderPreset } from "../../lib/provider-catalog";
import { useI18n } from "../../i18n";
import { Select } from "../Select";

export function ProviderPresetField({ app, disabled, onSelect }: {
  app: AppKind; disabled: boolean; onSelect: (preset: ProviderPreset) => void;
}) {
  const { t } = useI18n();
  const presets = providerPresets(app);
  return <div className="asb-field">
    <span>{t("providerSetup.preset")}</span>
    <Select ariaLabel={t("providerSetup.preset")} value={null} disabled={disabled}
      placeholder={t("providerSetup.presetPlaceholder")}
      options={presets.map(({ id, name }) => ({ value: id, label: name }))}
      onChange={(id) => { const preset = presets.find((item) => item.id === id); if (preset) onSelect(preset); }} />
    <p className="asb-field-help">{t("providerSetup.catalogDate", { date: providerCatalog.generatedAt })}</p>
  </div>;
}
