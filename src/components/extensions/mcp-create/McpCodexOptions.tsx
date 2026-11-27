import { useI18n } from "../../../i18n";
import type { CodexServerOptions } from "../../../api/client";
import { Input } from "../../Input";
import { Select } from "../../Select";
import { SettingsRow } from "../../SettingsLayout";

export function McpCodexOptions({ value, busy, onChange }: {
  value?: CodexServerOptions; busy: boolean; onChange: (value: CodexServerOptions | undefined) => void;
}) {
  const { t } = useI18n();
  const update = (key: keyof CodexServerOptions, next: string | number | boolean | undefined) => {
    const options = Object.fromEntries(Object.entries({ ...value, [key]: next }).filter(([, entry]) => entry != null));
    onChange(Object.keys(options).length ? options : undefined);
  };
  return <div className="asb-mcp-wizard-section" role="group" aria-label={t("mcp.codex.title")}>
    <div className="asb-mcp-field-heading"><span>{t("mcp.codex.title")}</span></div>
    <SettingsRow label={t("mcp.codex.cwd")}>
      <Input code aria-label={t("mcp.codex.cwd")} value={value?.cwd ?? ""} placeholder={t("mcp.placeholder.optional")} disabled={busy}
        onChange={(event) => update("cwd", event.target.value || undefined)} />
    </SettingsRow>
    {([["startupTimeoutSec", "mcp.codex.startupTimeout"], ["toolTimeoutSec", "mcp.codex.toolTimeout"]] as const).map(([key, labelKey]) =>
      <SettingsRow key={key} label={t(labelKey)}>
        <Input type="number" aria-label={t(labelKey)} min={0} step={1} max={Number.MAX_SAFE_INTEGER}
          value={value?.[key] ?? ""} disabled={busy}
          onChange={(event) => update(key, event.target.value === "" ? undefined : Number(event.target.value))} />
      </SettingsRow>)}
    <SettingsRow label={t("mcp.codex.required")}>
      <Select ariaLabel={t("mcp.codex.required")} value={value?.required == null ? "" : String(value.required)} disabled={busy}
        options={[{ value: "", label: t("mcp.option.unset") }, { value: "true", label: t("mcp.option.yes") }, { value: "false", label: t("mcp.option.no") }]}
        onChange={(next) => update("required", next === "" ? undefined : next === "true")} />
    </SettingsRow>
  </div>;
}
