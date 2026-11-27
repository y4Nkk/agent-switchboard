import type { ReactNode } from "react";
import { useI18n } from "../../i18n";
import { SettingsGroup } from "../SettingsLayout";

interface Props {
  children: ReactNode;
}

export function ProviderAdvancedSettings({ children }: Props) {
  const { t } = useI18n();
  return (
    <SettingsGroup title={t("providers.editor.section.advanced")}>
      <div className="asb-editor-section-fields asb-provider-advanced-content">{children}</div>
    </SettingsGroup>
  );
}
