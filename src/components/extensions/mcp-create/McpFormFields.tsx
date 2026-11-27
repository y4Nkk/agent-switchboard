import { WandSparkles } from "lucide-react";
import { useI18n } from "../../../i18n";
import type { AppKind } from "../../../api/client";
import type { McpRegistrySource } from "../../../api/extensions/mcp";
import { clientName } from "../../../lib/client-name";
import { Button } from "../../Button";
import { Checkbox } from "../../Checkbox";
import { Input } from "../../Input";
import { Select } from "../../Select";
import { SettingsGroup, SettingsRow } from "../../SettingsLayout";
import { Textarea } from "../../Textarea";
import { Tooltip } from "../../Tooltip";
import { MANAGEMENT_CLIENTS } from "../client-presentation";
import { mcpPresetOptions } from "./presets";

interface IdentityProps {
  name: string;
  busy: boolean;
  registry?: McpRegistrySource | null;
  preset?: { value: string; onChange: (value: string) => void };
  autoFocus?: boolean;
  onNameChange: (name: string) => void;
}

export function McpIdentityFields({ name, preset, registry, busy, autoFocus, onNameChange }: IdentityProps) {
  const { t } = useI18n();
  const nameLabel = t("mcp.field.name");
  const nameHint = t("mcp.field.nameHint");
  return (
    <>
      {registry && <SettingsRow label={t("mcp.field.registrySource")}>
        <code className="asb-mcp-registry-source">{t("extensions.registry.sourceVersion", { name: registry.name, version: registry.version })}</code>
      </SettingsRow>}
      <SettingsRow label={nameLabel} description={nameHint}>
        <Input code aria-label={nameLabel} aria-description={nameHint} value={name} autoFocus={autoFocus}
          placeholder="docs-search" disabled={busy} onChange={(event) => onNameChange(event.target.value)} />
      </SettingsRow>
      {preset && <SettingsRow label={t("mcp.field.preset")}>
        <Select ariaLabel={t("mcp.field.presetAria")} value={preset.value} options={mcpPresetOptions()}
          disabled={busy} onChange={preset.onChange} />
      </SettingsRow>}
    </>
  );
}

interface ClientProps {
  clients: AppKind[];
  busy: boolean;
  onToggle: (client: AppKind) => void;
}

export function McpClientFields({ clients, busy, onToggle }: ClientProps) {
  const { t } = useI18n();
  return (
    <SettingsGroup title={t("mcp.clients.label")}>
      <div className="asb-mcp-clients" role="group" aria-label={t("mcp.clients.aria")}>
        {MANAGEMENT_CLIENTS.map((client) => {
          const selected = clients.includes(client);
          return (
            <Tooltip key={client} label={t("mcp.client.tooltip", { client: clientName(client),
              state: selected ? t("mcp.client.stateOn") : t("mcp.client.stateOff") })}>
              <Checkbox label={clientName(client)} checked={selected} disabled={busy}
                ariaLabel={t("mcp.client.enableAria", { client: clientName(client) })}
                onChange={() => onToggle(client)} />
            </Tooltip>
          );
        })}
      </div>
    </SettingsGroup>
  );
}

interface JsonProps {
  value: string;
  busy: boolean;
  focus: boolean;
  error: string | null;
  errorId: string;
  onChange: (json: string) => void;
  onWizard: () => void;
}

export function McpJsonField({ value, busy, focus, error, errorId, onChange, onWizard }: JsonProps) {
  const { t } = useI18n();
  return (
    <SettingsGroup title={t("mcp.group.connection")} actions={
      <Button variant="secondary" disabled={busy} onClick={onWizard}>
        <WandSparkles size={16} aria-hidden="true" />{t("mcp.field.wizard")}
      </Button>
    }>
      <SettingsRow label={t("mcp.field.json")} wide>
        <div className="asb-mcp-json-field">
          <Textarea code aria-label={t("mcp.field.jsonAria")} value={value} rows={12} spellCheck={false}
            autoComplete="off" autoFocus={focus} disabled={busy} aria-invalid={Boolean(error)}
            aria-describedby={error ? errorId : undefined} aria-errormessage={error ? errorId : undefined}
            placeholder={'{\n  "command": "uvx",\n  "args": ["mcp-server-fetch"]\n}'}
            onChange={(event) => onChange(event.target.value)} />
          {error && <p id={errorId} className="asb-warn-text asb-mcp-error" role="alert">{error}</p>}
        </div>
      </SettingsRow>
    </SettingsGroup>
  );
}
