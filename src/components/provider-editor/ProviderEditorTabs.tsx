import { useI18n } from "../../i18n";
import { Tabs } from "../Tabs";
import type { ProviderEditorSection } from "./useProviderEditorSections";

export function ProviderEditorTabs({ section, onChange, scope, busy, official }: {
  section: ProviderEditorSection; onChange: (section: ProviderEditorSection) => void;
  scope: string; busy: boolean; official: boolean;
}) {
  const { t } = useI18n();
  const sections: ProviderEditorSection[] = official ? ["connection", "parameters"] : ["connection", "models", "parameters"];
  return <Tabs value={section} onChange={onChange} scope={scope} label={t("providerEditor.sections")}
    tabs={sections.map((value) => ({ value, label: t(`providerEditor.section.${value}`),
      controls: `${scope}-${value}-panel`, disabled: busy }))} />;
}
