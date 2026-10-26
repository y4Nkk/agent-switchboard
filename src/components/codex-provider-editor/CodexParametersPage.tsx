import type { SettingSpec, SettingValue } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { SettingsFields, SettingsRow } from "../SettingsFields";
import { ParametersLoadStatus } from "../provider-editor/ProviderParametersPage";
import type { CodexEditorState } from "./useCodexProviderEditor";

interface Props {
  editor: CodexEditorState;
  busy: boolean;
  baselineValues?: Record<string, SettingValue>;
}

const automatic: SettingValue = { mode: "automatic" };

function SubagentSettingsGroup({ specs, baselineValues, editor, busy, onChange }: {
  specs: SettingSpec[];
  baselineValues: Record<string, SettingValue>;
  editor: CodexEditorState;
  busy: boolean;
  onChange: (key: string, value: SettingValue) => void;
}) {
  const { t } = useI18n();
  if (!editor.draft.parameters) return null;
  const settings = editor.draft.parameters.settings;
  return (
    <section className="asb-toggle-group">
      <div className="asb-toggle-group-head">
        <h3 className="asb-section-title">{t("codex.params.subagent")}</h3>
      </div>
      <p className="asb-field-help">{t("codex.params.subagentNote")}</p>
      {specs.map((spec) => (
        <SettingsRow key={spec.key} spec={spec} value={settings[spec.key]}
          baselineValue={baselineValues[spec.key] ?? automatic} showActual={false} busy={busy}
          presentation="provider" onChange={(value) => onChange(spec.key, value)} />
      ))}
    </section>
  );
}

export function CodexParametersPage({ editor, busy, baselineValues }: Props) {
  const { t } = useI18n();
  const { draft, setDraft, parameters } = editor;
  const catalog = parameters.catalog;
  if (!catalog || !draft.parameters) return <ParametersLoadStatus busy={busy}
    ready={parameters.ready} error={parameters.error} retry={parameters.retry} />;
  const resetAll = () => setDraft((current) => current.parameters
    ? { ...current, parameters: { settings: { ...catalog.defaults.settings } } }
    : current);
  const baseline = baselineValues ?? catalog.defaults.settings;
  const subagentSpecs = catalog.specs.filter((spec) => spec.group === "ownership.group.subagent");
  const parameterSpecs = catalog.specs.filter((spec) => spec.group !== "ownership.group.subagent");
  const parameterGroups = catalog.groups.filter((group) => group !== "ownership.group.subagent");
  const change = (key: string, value: SettingValue) => setDraft((current) => current.parameters
    ? { ...current, parameters: { settings: { ...current.parameters.settings, [key]: value } } }
    : current);
  return (
    <div className="asb-form asb-provider-parameters" aria-label={t("codex.params.pageAria")}>
      <p className="asb-field-help">{t("codex.params.pageNote")}</p>
      <SettingsFields specs={parameterSpecs} groups={parameterGroups} values={draft.parameters.settings}
        baselineValues={baseline} busy={busy} showGroupReset={false} presentation="provider" onChange={change} />
      <SubagentSettingsGroup specs={subagentSpecs} baselineValues={baseline}
        editor={editor} busy={busy} onChange={change} />
      <div className="asb-settings-actions">
        <Button variant="secondary" disabled={busy} onClick={resetAll}>{t("codex.params.reset")}</Button>
      </div>
    </div>
  );
}
