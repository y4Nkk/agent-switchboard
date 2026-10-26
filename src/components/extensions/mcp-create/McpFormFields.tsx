import { WandSparkles } from "lucide-react";
import { useI18n } from "../../../i18n";
import type { AppKind } from "../../../api/client";
import { clientName } from "../../../lib/client-name";
import { Button } from "../../Button";
import { Checkbox } from "../../Checkbox";
import { Input } from "../../Input";
import { Select } from "../../Select";
import { Textarea } from "../../Textarea";
import { Tooltip } from "../../Tooltip";
import { MANAGEMENT_CLIENTS } from "../client-presentation";
import { mcpPresetOptions } from "./presets";

interface IdentityProps {
  name: string;
  preset: string;
  busy: boolean;
  onNameChange: (name: string) => void;
  onPresetChange: (preset: string) => void;
}

export function McpIdentityFields({ name, preset, busy, onNameChange, onPresetChange }: IdentityProps) {
  const { t } = useI18n();
  return (
    <div className="asb-mcp-identity">
      <label className="asb-field">
        <span>{t("mcp.field.name")}</span>
        <Input code aria-label={t("mcp.field.name")} value={name} placeholder="docs-search" disabled={busy}
          title={t("mcp.field.nameHint")} onChange={(event) => onNameChange(event.target.value)} />
      </label>
      <div className="asb-field">
        <span>{t("mcp.field.preset")}</span>
        <Select ariaLabel={t("mcp.field.presetAria")} value={preset} options={mcpPresetOptions()}
          disabled={busy} onChange={onPresetChange} />
      </div>
    </div>
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
    <div className="asb-mcp-client-field" role="group" aria-label={t("mcp.clients.aria")}>
      <span>{t("mcp.clients.label")}</span>
      <div className="asb-mcp-clients">
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
    </div>
  );
}

interface JsonProps {
  value: string;
  busy: boolean;
  focus: boolean;
  invalid: boolean;
  errorId: string;
  onChange: (json: string) => void;
  onWizard: () => void;
}

export function McpJsonField({ value, busy, focus, invalid, errorId, onChange, onWizard }: JsonProps) {
  const { t } = useI18n();
  return (
    <div className="asb-mcp-json-field">
      <div className="asb-mcp-field-heading">
        <span>{t("mcp.field.json")}</span>
        <Button variant="secondary" disabled={busy} onClick={onWizard}>
          <WandSparkles size={16} aria-hidden="true" />{t("mcp.field.wizard")}
        </Button>
      </div>
      <Textarea code aria-label={t("mcp.field.jsonAria")} value={value} rows={12} spellCheck={false}
        autoComplete="off" autoFocus={focus} disabled={busy} aria-invalid={invalid}
        aria-describedby={invalid ? errorId : undefined} aria-errormessage={invalid ? errorId : undefined}
        placeholder={'{\n  "command": "uvx",\n  "args": ["mcp-server-fetch"]\n}'}
        onChange={(event) => onChange(event.target.value)} />
    </div>
  );
}
