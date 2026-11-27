import { useId, useState } from "react";
import type { AppKind, ConfigValue, SettingSpec, SettingValue } from "../api/client";
import { useI18n, type TFunction } from "../i18n";
import { catalogText } from "../i18n/errors";
import { sameSettingValue } from "../lib/setting-values";
import { Button } from "./Button";
import { Collapsible } from "./Collapsible";
import { CodeIcon } from "./icons";
import { RadioOption } from "./RadioOption";
import { ScrollArea } from "./ScrollArea";
import { Select } from "./Select";
import { SettingInput } from "./SettingInput";
import { SettingsGroup, SettingsRow } from "./SettingsLayout";
import { Slider } from "./Slider";
import { Tooltip } from "./Tooltip";
import { settingDetail } from "./setting-detail";

interface Props {
  specs: SettingSpec[];
  groups: string[];
  values: Record<string, SettingValue>;
  baselineValues?: Record<string, SettingValue>;
  actualValues?: Record<string, SettingValue>;
  supportedSettings?: readonly string[];
  savedValues?: Record<string, SettingValue>;
  keepFileValues?: readonly string[];
  busy: boolean;
  onChange: (key: string, value: SettingValue) => void;
  onKeepFileValue?: (key: string) => void;
  clientApp?: AppKind;
}

const automatic: SettingValue = { mode: "automatic" };
function explicit(value: boolean | string): SettingValue { return { mode: "explicit", value }; }

function choiceIndex(spec: SettingSpec, value: SettingValue): number {
  return value.mode === "automatic" ? 0 : spec.options.findIndex((option) => option.value === value.value) + 1;
}

function choiceLabel(spec: SettingSpec, value: SettingValue, t: TFunction): string {
  if (spec.control === "approvalPolicy" && value.mode === "explicit"
    && typeof value.value === "object" && !Array.isArray(value.value)) {
    const fields = value.value.granular;
    if (typeof fields === "object" && !Array.isArray(fields)) return t("clientConfig.settings.granularSummary", {
      enabled: Object.values(fields).filter((enabled) => enabled === true).length, total: spec.options.length,
    });
  }
  if (spec.control === "toggle" && value.mode === "explicit") {
    return t(value.value === true ? "clientConfig.settings.on" : "clientConfig.settings.off");
  }
  return value.mode === "automatic"
    ? t("clientConfig.settings.automatic")
    : catalogText(spec.options.find((option) => option.value === value.value)?.label ?? "", t)
      || (value.value === "" ? t("clientConfig.settings.emptyText")
        : typeof value.value === "object" ? JSON.stringify(value.value) : String(value.value));
}

interface SettingControlProps {
  spec: SettingSpec;
  value: SettingValue;
  busy: boolean;
  onChange: (value: SettingValue) => void;
  clientApp?: AppKind;
}

function SettingControl({ spec, value, busy, onChange, clientApp }: SettingControlProps) {
  const { t } = useI18n();
  const name = useId();
  const label = catalogText(spec.label, t);
  if (spec.control === "slider") {
    const current = choiceLabel(spec, value, t);
    return <div className="asb-slider-control">
      <span className="asb-choice-current" aria-live="polite">{t("clientConfig.settings.currentEffort", { value: current })}</span>
      <Slider value={choiceIndex(spec, value)} min={0} max={spec.options.length} step={1}
        ariaLabel={label} ariaValueText={label + " " + current} disabled={busy}
        onValueChange={(index) => onChange(index === 0 ? automatic : explicit(spec.options[index - 1].value))} />
    </div>;
  }
  if (clientApp && spec.control === "segment") {
    const selected = choiceIndex(spec, value);
    return <Select ariaLabel={label} disabled={busy}
      value={selected || value.mode === "automatic" ? String(selected) : null}
      placeholder={choiceLabel(spec, value, t)}
      options={[
        { value: "0", label: t("clientConfig.settings.notSet") },
        ...spec.options.map((option, index) => ({ value: String(index + 1), label: catalogText(option.label, t) })),
      ]}
      onChange={(index) => onChange(index === "0" ? automatic : explicit(spec.options[Number(index) - 1].value))} />;
  }
  if (spec.control !== "toggle" && spec.control !== "segment") {
    return <SettingInput spec={spec} value={value} busy={busy} onChange={onChange} />;
  }
  const options = spec.control === "toggle"
    ? [{ value: true, label: t("clientConfig.settings.on") }, { value: false, label: t("clientConfig.settings.off") }]
    : spec.options;
  return (
    <div className="asb-segments" role="radiogroup" aria-label={label}>
      <RadioOption name={name} checked={value.mode === "automatic"} disabled={busy}
        label={t(clientApp ? "clientConfig.settings.removeUserValue" : "clientConfig.settings.automatic")} onChange={() => onChange(automatic)} />
      {options.map((option) => (
        <RadioOption key={String(option.value)} name={name}
          checked={value.mode === "explicit" && value.value === option.value}
          disabled={busy} label={catalogText(option.label, t)} onChange={() => onChange(explicit(option.value))} />
      ))}
    </div>
  );
}

function preferenceDetail(spec: SettingSpec, t: TFunction): string | undefined {
  if (spec.key === "autoMemoryDirectory") return t("clientConfig.settings.memoryDirectoryHelp");
  if (spec.control === "notifications") return t("clientConfig.settings.notificationsHelp");
  if (spec.control === "stringList") return settingDetail(spec.key, t) ?? t("clientConfig.settings.listHelp");
  return settingDetail(spec.key, t);
}

function providerParameterDetail(spec: SettingSpec, t: TFunction): string | undefined {
  const detail = settingDetail(spec.key, t);
  if (detail) return detail;
  if (spec.key === "alwaysThinkingEnabled") return t("clientConfig.settings.thinkingHelp");
  if (spec.key === "service_tier") return t("clientConfig.settings.serviceTierHelp");
  if (spec.key === "fastMode") return t("clientConfig.settings.fastModeHelp");
  if (spec.key === "env.CLAUDE_CODE_MAX_OUTPUT_TOKENS") return t("clientConfig.settings.outputLimitHelp");
  return undefined;
}

function wideControl(spec: SettingSpec, value: SettingValue): boolean {
  if (value.mode !== "explicit") return false;
  return spec.control === "stringList" || spec.control === "multilineText" || spec.control === "editableChoice"
    || (spec.control === "notifications" && Array.isArray(value.value))
    || (spec.control === "approvalPolicy" && typeof value.value === "object");
}

function scalarCode(value: ConfigValue): string {
  if (Array.isArray(value)) return "[" + value.map(scalarCode).join(", ") + "]";
  if (typeof value === "object") {
    return "{ " + Object.entries(value).map(([key, item]) => JSON.stringify(key) + " = " + scalarCode(item)).join(", ") + " }";
  }
  return typeof value === "string" ? JSON.stringify(value) : String(value);
}

function settingCode(app: AppKind, key: string, value: SettingValue | undefined, t: TFunction): string {
  if (!value || value.mode === "automatic") return t("clientConfig.settings.codeNotSet", { key });
  const path = key.split(".");
  if (app === "codex") {
    const property = path.at(-1)!;
    const table = path.slice(0, -1);
    return (table.length > 0 ? "[" + table.join(".") + "]\n" : "") + property + " = " + scalarCode(value.value);
  }
  const root: Record<string, unknown> = {};
  let target = root;
  for (const segment of path.slice(0, -1)) {
    const nested: Record<string, unknown> = {};
    target[segment] = nested;
    target = nested;
  }
  target[path.at(-1)!] = value.value;
  return JSON.stringify(root, null, 2);
}

function CurrentSettingCode({ app, spec, actualValue, available }: {
  app: AppKind;
  spec: SettingSpec;
  actualValue?: SettingValue;
  available: boolean;
}) {
  const { t } = useI18n();
  const code = available ? settingCode(app, spec.key, actualValue, t) : t("clientConfig.settings.codeUnreadable", { key: spec.key });
  return <ScrollArea className="asb-client-setting-code-body" tabIndex={0}
    aria-label={t("clientConfig.settings.viewCurrentCode", { label: catalogText(spec.label, t) })}>
    <code>{code}</code>
  </ScrollArea>;
}

interface ClientPreferenceRowProps extends SettingControlProps {
  clientApp: AppKind;
  baselineValue?: SettingValue;
  actualValue?: SettingValue;
  showActual: boolean;
  versionUnavailable: boolean;
  keepFileValuePending?: boolean;
  onKeepFileValue?: () => void;
}

function ClientPreferenceRow({ spec, value, baselineValue, actualValue, showActual, busy, onChange, clientApp,
  versionUnavailable, keepFileValuePending, onKeepFileValue }: ClientPreferenceRowProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const codeId = useId();
  const label = catalogText(spec.label, t);
  const codeLabel = t("clientConfig.settings.viewCurrentCode", { label });
  const canUndo = baselineValue !== undefined && !sameSettingValue(baselineValue, value);
  return (
    <SettingsRow label={label} description={preferenceDetail(spec, t)} wide={wideControl(spec, value)}
      labelAction={<Tooltip label={codeLabel} asChild>
        <Button variant="icon" size="compact" className="asb-client-setting-code-toggle" aria-label={codeLabel}
          aria-expanded={open} aria-controls={open ? codeId : undefined} onClick={() => setOpen(!open)}>
          <CodeIcon />
        </Button>
      </Tooltip>}
      details={<>
        {versionUnavailable && spec.minimumVersion && <p className="asb-field-help">
          {t("clientConfig.settings.versionUnverified", { version: spec.minimumVersion })}
        </p>}
        <Collapsible open={open} id={codeId}>
          <CurrentSettingCode app={clientApp} spec={spec} actualValue={actualValue} available={showActual} />
        </Collapsible>
      </>}>
      <SettingControl spec={spec} value={value} busy={busy || versionUnavailable} onChange={onChange} clientApp={clientApp} />
      {versionUnavailable && actualValue?.mode === "explicit" && <Button variant="secondary" size="compact" disabled={busy}
        onClick={() => onChange(automatic)}>{t("clientConfig.settings.removeUserValue")}</Button>}
      {onKeepFileValue && <Tooltip asChild enabled={keepFileValuePending === true} label={t("clientConfig.settings.pendingKeepFile")}>
        <Button variant="secondary" size="compact" disabled={busy} onClick={onKeepFileValue}>
          {t(keepFileValuePending ? "clientConfig.settings.undoChange" : "clientConfig.settings.releaseToFile")}
        </Button>
      </Tooltip>}
      {canUndo && <Button variant="secondary" size="compact" disabled={busy} onClick={() => onChange(baselineValue)}>
        {t("clientConfig.settings.undoChange")}
      </Button>}
    </SettingsRow>
  );
}

function pendingParameterMessage(spec: SettingSpec, value: SettingValue, t: TFunction): string {
  if (value.mode === "automatic") return t("clientConfig.settings.pendingSaveRemoved");
  return t(spec.control === "toggle" ? "clientConfig.settings.pendingSaveToggle" : "clientConfig.settings.pendingSaveWrite",
    { value: choiceLabel(spec, value, t) });
}

export function ProviderParameterRow({ spec, value, baselineValue = automatic, busy, onChange }: SettingControlProps & {
  baselineValue?: SettingValue;
}) {
  const { t } = useI18n();
  const changed = !sameSettingValue(baselineValue, value);
  const description = providerParameterDetail(spec, t);
  return (
    <SettingsRow label={catalogText(spec.label, t)} wide={wideControl(spec, value)}
      description={<>
        {description && <span>{description}</span>}
        <span className="asb-setting-actual">{t("clientConfig.settings.currentSetting", { value: choiceLabel(spec, baselineValue, t) })}</span>
      </>}
      details={changed && <p className="asb-setting-pending" role="status">{pendingParameterMessage(spec, value, t)}</p>}>
      <SettingControl spec={spec} value={value} busy={busy} onChange={onChange} />
    </SettingsRow>
  );
}

/** The setting catalog supplies the groups, values and editor controls for both ownership domains. */
export function SettingsFields({
  specs, groups, values, baselineValues, actualValues, supportedSettings,
  savedValues, keepFileValues, onKeepFileValue, busy, onChange, clientApp,
}: Props) {
  const { t } = useI18n();
  return (
    <div className="asb-settings-groups">
      {groups.map((group) => {
        const groupSpecs = specs.filter((spec) => spec.group === group);
        if (groupSpecs.length === 0) return null;
        return (
          <SettingsGroup key={group} title={catalogText(group, t)}
            description={group === "ownership.group.sandbox" ? t("clientConfig.settings.sandboxPlatform") : undefined}>
            {groupSpecs.map((spec) => {
              const props = { spec, value: values[spec.key] ?? automatic, baselineValue: baselineValues?.[spec.key],
                busy, onChange: (next: SettingValue) => onChange(spec.key, next) };
              return clientApp ? <ClientPreferenceRow key={spec.key} {...props} clientApp={clientApp}
                actualValue={actualValues?.[spec.key]} showActual={actualValues !== undefined}
                versionUnavailable={!!spec.minimumVersion && !supportedSettings?.includes(spec.key)}
                keepFileValuePending={keepFileValues?.includes(spec.key)}
                onKeepFileValue={onKeepFileValue && savedValues?.[spec.key]?.mode === "explicit" ? () => onKeepFileValue(spec.key) : undefined} />
                : <ProviderParameterRow key={spec.key} {...props} />;
            })}
          </SettingsGroup>
        );
      })}
    </div>
  );
}
