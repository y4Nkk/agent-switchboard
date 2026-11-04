import { useState } from "react";
import type { AppKind, KeyChange, SettingSpec, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import type { TFunction } from "../i18n";
import { Button } from "./Button";
import { CollapsibleSection } from "./Collapsible";
import { DiffView } from "./DiffView";
import { RadioOption } from "./RadioOption";
import { Slider } from "./Slider";
import { SettingInput } from "./SettingInput";
import { ScrollArea } from "./ScrollArea";
import { settingDetail } from "./setting-detail";
import { sameSettingValue } from "../lib/setting-values";

interface Props {
  specs: SettingSpec[];
  groups: string[];
  values: Record<string, SettingValue>;
  /** Saved values used to summarize the current setting and derive a pending
   * change in client-preference and provider-parameter presentations. */
  baselineValues?: Record<string, SettingValue>;
  /** Read-only values currently present in the real client file. */
  actualValues?: Record<string, SettingValue>;
  busy: boolean;
  onChange: (key: string, value: SettingValue) => void;
  onResetGroup?: (group: string | null) => void;
  showGroupReset?: boolean;
  /** Client preferences and provider parameters share the richer decision-row
   * anatomy; all other catalog consumers retain compact form rows. */
  presentation?: "client" | "provider";
  /** The official client file whose current managed field is shown beside a
   * client-preference control. */
  clientApp?: AppKind;
}

const automatic: SettingValue = { mode: "automatic" };
function explicit(value: boolean | string): SettingValue { return { mode: "explicit", value }; }

function choiceIndex(spec: SettingSpec, value: SettingValue): number {
  return value.mode === "automatic" ? 0 : spec.options.findIndex((option) => option.value === value.value) + 1;
}

export function choiceLabel(spec: SettingSpec, value: SettingValue, t: TFunction): string {
  if (spec.control === "approvalPolicy" && value.mode === "explicit"
    && typeof value.value === "object" && !Array.isArray(value.value)) {
    const fields = value.value.granular;
    if (typeof fields === "object" && !Array.isArray(fields)) return t("clientConfig.settings.granularSummary", {
      enabled: Object.values(fields).filter((enabled) => enabled === true).length, total: spec.options.length,
    });
  }
  return value.mode === "automatic"
    ? t("clientConfig.settings.automatic")
    : catalogText(spec.options.find((option) => option.value === value.value)?.label ?? "", t)
      || (value.value === "" ? t("clientConfig.settings.emptyText")
        : typeof value.value === "object" ? JSON.stringify(value.value) : String(value.value));
}

function diffValue(value: SettingValue): string | null {
  return value.mode === "automatic" ? null : typeof value.value === "object" ? JSON.stringify(value.value) : String(value.value);
}

function actualValueLabel(spec: SettingSpec, value: SettingValue | undefined, t: TFunction): string {
  if (!value) return t("clientConfig.settings.actualUnreadable");
  return t("clientConfig.settings.actualValue", { value: choiceLabel(spec, value, t) });
}

function settingChange(
  spec: SettingSpec,
  baselineValue: SettingValue | undefined,
  value: SettingValue,
): KeyChange | null {
  if (!baselineValue || sameSettingValue(baselineValue, value)) return null;
  return {
    key: spec.key,
    kind: value.mode === "automatic" ? "remove" : "set",
    before: diffValue(baselineValue),
    after: diffValue(value),
  };
}

interface ControlProps {
  spec: SettingSpec;
  value: SettingValue;
  baselineValue?: SettingValue;
  actualValue?: SettingValue;
  showActual: boolean;
  busy: boolean;
  onChange: (value: SettingValue) => void;
  presentation?: "client" | "provider";
  clientApp?: AppKind;
}

type SettingControlProps = Pick<ControlProps, "spec" | "value" | "busy" | "onChange">;
type ProviderParameterRowProps = Pick<ControlProps, "spec" | "value" | "baselineValue" | "busy" | "onChange">;

function SettingControl({ spec, value, busy, onChange }: SettingControlProps) {
  const { t } = useI18n();
  if (spec.control !== "toggle" && spec.control !== "slider" && spec.control !== "segment") {
    return <SettingInput spec={spec} value={value} busy={busy} onChange={onChange} />;
  }
  if (spec.control === "slider") {
    return (
      <div className="asb-slider-control">
        <span className="asb-choice-current" aria-live="polite">{t("clientConfig.settings.currentEffort", { value: choiceLabel(spec, value, t) })}</span>
        <Slider value={choiceIndex(spec, value)} min={0} max={spec.options.length} step={1}
          ariaLabel={catalogText(spec.label, t)} ariaValueText={`${catalogText(spec.label, t)} ${choiceLabel(spec, value, t)}`} disabled={busy}
          onValueChange={(index) => onChange(index === 0 ? automatic : explicit(spec.options[index - 1].value))} />
      </div>
    );
  }
  const options = spec.control === "toggle"
    ? [{ value: true, label: t("clientConfig.settings.on") }, { value: false, label: t("clientConfig.settings.off") }]
    : spec.options;
  return (
    <div className="asb-segments" role="radiogroup" aria-label={catalogText(spec.label, t)}>
      <RadioOption name={`${spec.key}-setting`} checked={value.mode === "automatic"} disabled={busy}
        label={t("clientConfig.settings.automatic")} onChange={() => onChange(automatic)} />
      {options.map((option) => (
        <RadioOption key={String(option.value)} name={`${spec.key}-setting`}
          checked={value.mode === "explicit" && value.value === option.value}
          disabled={busy} label={catalogText(option.label, t)} onChange={() => onChange(explicit(option.value))} />
      ))}
    </div>
  );
}

function preferenceDetail(spec: SettingSpec, t: TFunction): string {
  const detail = settingDetail(spec.key, t);
  if (detail) return detail;
  if (spec.key === "autoMemoryDirectory") return t("clientConfig.settings.memoryDirectoryHelp");
  if (spec.control === "text" || spec.control === "integer" || spec.control === "stringInteger") return t("clientConfig.settings.preferenceDetailInput");
  if (spec.control === "notifications") return t("clientConfig.settings.notificationsHelp");
  if (spec.control === "stringList") return t("clientConfig.settings.listHelp");
  if (spec.control === "toggle") return t("clientConfig.settings.preferenceDetailToggle");
  if (spec.control === "slider") return t("clientConfig.settings.preferenceDetailSlider");
  return t("clientConfig.settings.preferenceDetailChoice");
}

function providerParameterDetail(spec: SettingSpec, t: TFunction): string {
  const detail = settingDetail(spec.key, t);
  if (detail) return detail;
  if (spec.key === "alwaysThinkingEnabled") return t("clientConfig.settings.thinkingHelp");
  if (spec.key === "service_tier") return t("clientConfig.settings.serviceTierHelp");
  if (spec.key === "fastMode") return t("clientConfig.settings.fastModeHelp");
  if (spec.key === "env.CLAUDE_CODE_MAX_OUTPUT_TOKENS") return t("clientConfig.settings.outputLimitHelp");
  if (spec.control === "text" || spec.control === "integer" || spec.control === "stringInteger") return t("clientConfig.settings.parameterDetailInput");
  if (spec.control === "toggle") return t("clientConfig.settings.parameterDetailToggle");
  if (spec.control === "slider") return t("clientConfig.settings.parameterDetailSlider");
  return t("clientConfig.settings.parameterDetailChoice");
}

function pendingSettingMessage(spec: SettingSpec, value: SettingValue, apply: boolean, t: TFunction): string {
  if (value.mode === "automatic") {
    return apply ? t("clientConfig.settings.pendingApplyRemoved") : t("clientConfig.settings.pendingSaveRemoved");
  }
  const setValue = choiceLabel(spec, value, t);
  if (spec.control === "toggle") {
    return apply ? t("clientConfig.settings.pendingApplyToggle", { value: setValue }) : t("clientConfig.settings.pendingSaveToggle", { value: setValue });
  }
  return apply ? t("clientConfig.settings.pendingApplyWrite", { value: setValue }) : t("clientConfig.settings.pendingSaveWrite", { value: setValue });
}

function scalarCode(value: import("../api/client").ConfigValue): string {
  if (Array.isArray(value)) return `[${value.map(scalarCode).join(", ")}]`;
  if (typeof value === "object") return `{ ${Object.entries(value).map(([key, item]) => `${JSON.stringify(key)} = ${scalarCode(item)}`).join(", ")} }`;
  return typeof value === "string" ? JSON.stringify(value) : String(value);
}

function settingCode(app: AppKind, key: string, value: SettingValue | undefined, t: TFunction): string {
  if (!value || value.mode === "automatic") return t("clientConfig.settings.codeNotSet", { key });
  const path = key.split(".");
  if (app === "codex") {
    const property = path.at(-1)!;
    const table = path.slice(0, -1);
    return `${table.length > 0 ? `[${table.join(".")}]\n` : ""}${property} = ${scalarCode(value.value)}`;
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

function prefixedCode(prefix: string, content: string): string {
  return content.split("\n").map((line) => `${prefix} ${line}`).join("\n");
}

function SettingCodeDisclosure({
  app,
  spec,
  actualValue,
  draftValue,
  changed,
  available,
}: {
  app: AppKind;
  spec: SettingSpec;
  actualValue: SettingValue | undefined;
  draftValue: SettingValue;
  changed: boolean;
  available: boolean;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const actualCode = available ? settingCode(app, spec.key, actualValue, t) : t("clientConfig.settings.codeUnreadable", { key: spec.key });
  const draftCode = settingCode(app, spec.key, draftValue, t);
  const showDiff = changed && actualCode !== draftCode;
  return (
    <CollapsibleSection quiet open={open} onOpenChange={setOpen}
      label={showDiff ? t("clientConfig.settings.viewPendingDiff") : t("clientConfig.settings.viewCurrentCode")}>
      <ScrollArea className="asb-client-setting-code-body"
        aria-label={showDiff ? t("clientConfig.settings.pendingDiffAria", { label: catalogText(spec.label, t) }) : catalogText(spec.label, t)}>
        {showDiff ? (
          <code>
            <span className="asb-client-setting-code-old">{prefixedCode("-", actualCode)}</span>
            {"\n"}
            <span className="asb-client-setting-code-new">{prefixedCode("+", draftCode)}</span>
          </code>
        ) : (
          <code>{actualCode}</code>
        )}
      </ScrollArea>
    </CollapsibleSection>
  );
}

function ClientPreferenceRow({ spec, value, baselineValue, actualValue, showActual, busy, onChange, clientApp }: ControlProps) {
  const { t } = useI18n();
  const change = settingChange(spec, baselineValue, value);
  if (!clientApp) throw new Error("Client preference rows require a client application.");
  const actualLabel = showActual ? choiceLabel(spec, actualValue ?? automatic, t) : t("clientConfig.settings.unreadable");
  return (
    <div className="asb-toggle-row asb-choice-row">
      <div className="asb-choice-head">
        <div className="asb-app-setting-copy">
          <span className="asb-checkbox-label">{catalogText(spec.label, t)}</span>
          <span className="asb-app-setting-detail">{preferenceDetail(spec, t)}</span>
        </div>
        <span className="asb-setting-actual" aria-live="polite">{t("clientConfig.settings.currentConfig", { value: actualLabel })}</span>
      </div>
      <div className="asb-choice-controls">
        <SettingControl spec={spec} value={value} busy={busy} onChange={onChange} />
      </div>
      {change && (
        <p className="asb-setting-pending" role="status">
          {pendingSettingMessage(spec, value, true, t)}
        </p>
      )}
      <SettingCodeDisclosure app={clientApp} spec={spec} actualValue={actualValue}
        draftValue={value} changed={change !== null} available={showActual} />
    </div>
  );
}

function ProviderParameterRow({ spec, value, baselineValue, busy, onChange }: ProviderParameterRowProps) {
  const { t } = useI18n();
  const baseline = baselineValue ?? automatic;
  const changed = !sameSettingValue(baseline, value);
  return (
    <div className="asb-toggle-row asb-choice-row">
      <div className="asb-choice-head">
        <div className="asb-app-setting-copy">
          <span className="asb-checkbox-label">{catalogText(spec.label, t)}</span>
          <span className="asb-app-setting-detail">{providerParameterDetail(spec, t)}</span>
        </div>
        <span className="asb-setting-actual" aria-live="polite">{t("clientConfig.settings.currentSetting", { value: choiceLabel(spec, baseline, t) })}</span>
      </div>
      <div className="asb-choice-controls">
        <SettingControl spec={spec} value={value} busy={busy} onChange={onChange} />
      </div>
      {changed && <p className="asb-setting-pending" role="status">{pendingSettingMessage(spec, value, false, t)}</p>}
    </div>
  );
}

/** One catalog-driven settings row. */
export function SettingsRow({
  spec,
  value,
  baselineValue,
  actualValue,
  showActual,
  busy,
  onChange,
  presentation,
  clientApp,
}: ControlProps) {
  const { t } = useI18n();
  if (presentation === "client") return <ClientPreferenceRow spec={spec} value={value} baselineValue={baselineValue}
    actualValue={actualValue} showActual={showActual} busy={busy} onChange={onChange} clientApp={clientApp} />;
  if (presentation === "provider") return <ProviderParameterRow spec={spec} value={value}
    baselineValue={baselineValue} busy={busy} onChange={onChange} />;
  const change = settingChange(spec, baselineValue, value);
  return (
    <div className="asb-toggle-row asb-choice-row">
      <div className="asb-choice-head">
        <span className="asb-checkbox-label">{catalogText(spec.label, t)}</span>
        {showActual && <span className="asb-setting-actual" aria-live="polite">{actualValueLabel(spec, actualValue, t)}</span>}
      </div>
      <SettingControl spec={spec} value={value} busy={busy} onChange={onChange} />
      {change && (
        <div className="asb-setting-diff">
          <DiffView changes={[change]} label={t("clientConfig.settings.unsavedDiff", { label: catalogText(spec.label, t) })} />
        </div>
      )}
    </div>
  );
}

/** Both ownership domains edit catalog values through the same controls. */
export function SettingsFields({
  specs,
  groups,
  values,
  baselineValues,
  actualValues,
  busy,
  onChange,
  onResetGroup,
  showGroupReset = true,
  presentation,
  clientApp,
}: Props) {
  const { t } = useI18n();
  return (
    <div className="asb-toggle-list">
      {groups.map((group) => {
        const groupSpecs = specs.filter((spec) => spec.group === group);
        if (!groupSpecs.length) return null;
        return (
          <section className="asb-toggle-group" key={group}>
            <div className="asb-toggle-group-head">
              <h3 className="asb-section-title">{catalogText(group, t)}</h3>
              {showGroupReset && onResetGroup && (
                <Button variant="secondary" disabled={busy} onClick={() => onResetGroup(group)}>{t("clientConfig.settings.resetGroup")}</Button>
              )}
            </div>
            {group === "ownership.group.sandbox" && <p className="asb-field-help">{t("clientConfig.settings.sandboxPlatform")}</p>}
            {groupSpecs.map((spec) => (
              <SettingsRow key={spec.key} spec={spec} value={values[spec.key]}
                baselineValue={baselineValues?.[spec.key]} actualValue={actualValues?.[spec.key]}
                showActual={actualValues !== undefined} busy={busy} presentation={presentation}
                clientApp={clientApp} onChange={(next) => onChange(spec.key, next)} />
            ))}
          </section>
        );
      })}
    </div>
  );
}
