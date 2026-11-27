import { useId } from "react";
import type { CodexSubagentSettings, SettingValue } from "../api/client";
import type { CodexSubagentSettingsEditorState } from "../app/useCodexSubagentSettings";
import { useI18n, type TFunction } from "../i18n";
import { commandErrorText } from "../i18n/errors";
import { Button } from "./Button";
import { Input } from "./Input";
import { RadioOption } from "./RadioOption";
import { SettingsGroup, SettingsRow } from "./SettingsLayout";

type SubagentField = keyof CodexSubagentSettings;

interface Props {
  editorState: CodexSubagentSettingsEditorState;
  busy: boolean;
  onChange: (field: SubagentField, value: SettingValue) => void;
  onRetryLoad: () => void;
}

const automatic: SettingValue = { mode: "automatic" };

function explicit(value: boolean | string | number): SettingValue {
  return { mode: "explicit", value };
}

function draftIssue(settings: CodexSubagentSettings, t: TFunction): string | null {
  const concurrency = settings.maxConcurrentThreadsPerSession;
  if (concurrency.mode === "explicit" &&
    (typeof concurrency.value !== "number" || !Number.isSafeInteger(concurrency.value) || concurrency.value < 1)) {
    return t("codex.subagentPanel.concurrentInvalid");
  }
  for (const [labelKey, value] of [
    ["codex.subagentPanel.enabledLabel", settings.enabled],
    ["codex.subagentPanel.interruptRowLabel", settings.interruptMessage],
  ] as const) {
    if (value.mode === "explicit" && typeof value.value !== "boolean") {
      return t("codex.subagentPanel.booleanInvalid", { label: t(labelKey) });
    }
  }
  return null;
}

function BooleanRow({ label, description, field, value, disabled, onChange }: {
  label: string;
  description: string;
  field: SubagentField;
  value: SettingValue;
  disabled: boolean;
  onChange: Props["onChange"];
}) {
  const { t } = useI18n();
  const name = useId();
  return (
    <SettingsRow label={label} description={description}>
      <div className="asb-segments" role="radiogroup" aria-label={label}>
        <RadioOption name={name} checked={value.mode === "automatic"} disabled={disabled}
          label={t("codex.subagent.auto")} onChange={() => onChange(field, automatic)} />
        <RadioOption name={name} checked={value.mode === "explicit" && value.value === true}
          disabled={disabled} label={t("codex.subagentPanel.on")} onChange={() => onChange(field, explicit(true))} />
        <RadioOption name={name} checked={value.mode === "explicit" && value.value === false}
          disabled={disabled} label={t("codex.subagentPanel.off")} onChange={() => onChange(field, explicit(false))} />
      </div>
    </SettingsRow>
  );
}

function NumberRow({ value, disabled, onChange }: {
  value: SettingValue;
  disabled: boolean;
  onChange: Props["onChange"];
}) {
  const { t } = useI18n();
  const name = useId();
  const custom = value.mode === "explicit";
  const current = custom ? String(value.value) : "";
  const invalid = custom && (!Number.isSafeInteger(Number(current)) || Number(current) < 1 || !/^\d+$/.test(current));
  const change = (next: SettingValue) => onChange("maxConcurrentThreadsPerSession", next);
  const edit = (raw: string) => {
    const parsed = Number(raw);
    change(explicit(/^\d+$/.test(raw) && Number.isSafeInteger(parsed) ? parsed : raw));
  };
  return (
    <SettingsRow label={t("codex.subagentPanel.concurrentLabel")} description={t("codex.subagentPanel.concurrentDetail")}>
      <div className="asb-segments" role="radiogroup" aria-label={t("codex.subagentPanel.concurrentModeAria")}>
        <RadioOption name={name} checked={!custom} disabled={disabled} label={t("codex.subagent.auto")}
          onChange={() => change(automatic)} />
        <RadioOption name={name} checked={custom} disabled={disabled} label={t("codex.subagentPanel.specifyValue")}
          onChange={() => change(explicit(current))} />
      </div>
      {custom && <div className="asb-subagent-input">
        <Input aria-invalid={invalid || undefined} aria-label={t("codex.subagentPanel.concurrentValueAria")}
          autoComplete="off" inputMode="numeric" disabled={disabled} value={current}
          onChange={(event) => edit(event.target.value)} />
      </div>}
    </SettingsRow>
  );
}

export function CodexSubagentSettingsPanel({ editorState: state, busy, onChange, onRetryLoad }: Props) {
  const { t } = useI18n();
  const title = t("codex.subagentPanel.title");
  if (state.phase === "idle" || state.phase === "loading") {
    return (
      <SettingsGroup title={title}>
        <div className="asb-settings-skeleton" role="status" aria-label={t("codex.loading")}>
          <div className="asb-skeleton" />
          <div className="asb-skeleton" />
          <div className="asb-skeleton" />
        </div>
      </SettingsGroup>
    );
  }
  if (state.phase === "loadError" || !state.snapshot || !state.draft) {
    const error = state.error ? commandErrorText(state.error, t) : t("codex.subagentPanel.configUnavailable");
    return (
      <SettingsGroup title={title}>
        <div className="asb-empty" role="alert">
          <p>{t("codex.subagentPanel.loadError", { error })}</p>
          <Button variant="secondary" disabled={busy} onClick={onRetryLoad}>{t("codex.subagentPanel.retryLoad")}</Button>
        </div>
      </SettingsGroup>
    );
  }
  const issue = draftIssue(state.draft, t);
  return (
    <SettingsGroup title={title} description={t("codex.subagentPanel.defaultsHelp")}>
      <BooleanRow label={t("codex.subagentPanel.enabledLabel")} description={t("codex.subagentPanel.enabledDetail")}
        field="enabled" value={state.draft.enabled} disabled={busy} onChange={onChange} />
      <NumberRow value={state.draft.maxConcurrentThreadsPerSession} disabled={busy} onChange={onChange} />
      <BooleanRow label={t("codex.subagentPanel.interruptRowLabel")} description={t("codex.subagentPanel.interruptDetail")}
        field="interruptMessage" value={state.draft.interruptMessage} disabled={busy} onChange={onChange} />
      {issue && <p className="asb-field-error" role="alert">{issue}</p>}
    </SettingsGroup>
  );
}
