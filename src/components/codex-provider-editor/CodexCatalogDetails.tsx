import type { CodexReasoningLevel } from "../../api/client";
import { useI18n } from "../../i18n";
import { Table } from "../Table";
import { ProviderValue } from "../provider-editor/ProviderValue";
import { REASONING_LEVEL_LABELS, type EditableCodexCatalogEntry } from "./draft";

const flags = [
  ["functionTools", "codex.capability.functionTools"], ["customTools", "codex.capability.customTools"],
  ["toolSearch", "codex.capability.toolSearch"], ["reasoning", "codex.capability.reasoning"],
  ["images", "codex.model.imageInput"], ["compact", "codex.model.compact"],
] as const;

export function CodexCatalogDetails({ entry, busy, onChange }: {
  entry: EditableCodexCatalogEntry; busy: boolean; onChange: (patch: Partial<EditableCodexCatalogEntry>) => void;
}) {
  const { t } = useI18n();
  const levels = entry.supportedReasoningLevels;
  const setLevels = (raw: string) => {
    const next = raw.split(/[,，\s]+/).filter(Boolean) as CodexReasoningLevel[];
    onChange({ supportedReasoningLevels: next,
      defaultReasoningLevel: next.includes(entry.defaultReasoningLevel) ? entry.defaultReasoningLevel : next[0] ?? "none" });
  };
  return <section className="asb-provider-advanced-group" aria-label={t("providerSetup.modelCapabilities", { model: entry.id })}>
    <h4 className="asb-provider-advanced-heading">{t("providerSetup.modelCapabilities", { model: entry.id })}</h4>
    <Table ariaLabel={t("providerSetup.capabilities")} rows={[...flags]} rowKey={([key]) => key} columns={[
      { key: "name", header: t("providerSetup.parameter"), render: ([, label]) => t(label) },
      { key: "value", header: t("providerSetup.value"), render: ([key, label]) => <ProviderValue label={`${entry.id} ${t(label)}`}
        disabled={busy} value={String(entry[key])} display={t(entry[key] ? "providerSetup.enabled" : "providerSetup.disabled")}
        options={[{ value: "true", label: t("providerSetup.enabled") }, { value: "false", label: t("providerSetup.disabled") }]}
        onChange={(value) => onChange(key === "reasoning" && value === "false"
          ? { reasoning: false, supportedReasoningLevels: ["none"], defaultReasoningLevel: "none" }
          : { [key]: value === "true" })} /> },
    ]} />
    {entry.reasoning && <div className="asb-provider-field-grid">
      <div className="asb-field"><span>{t("providerSetup.levels")}</span>
        <ProviderValue label={t("providerSetup.levels")} value={levels.join(", ")} disabled={busy} required preserveText onChange={setLevels} />
        <p className="asb-field-help">{t("providerSetup.levelsHelp")}</p>
      </div>
      <div className="asb-field"><span>{t("codex.model.defaultLevel")}</span>
        <ProviderValue label={t("codex.model.defaultLevel")} value={entry.defaultReasoningLevel} disabled={busy}
          options={levels.map((level) => ({ value: level, label: REASONING_LEVEL_LABELS[level] ? t(REASONING_LEVEL_LABELS[level]) : level }))}
          onChange={(value) => onChange({ defaultReasoningLevel: value as CodexReasoningLevel })} />
      </div>
    </div>}
  </section>;
}
