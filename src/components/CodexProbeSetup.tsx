import { useId } from "react";
import { CUSTOM_PROBE_QUESTION_ID, type CodexProbeGradingVersion } from "../api/client";
import type { useCodexProbe } from "../app/useCodexProbe";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Input } from "./Input";
import { probeTime } from "./ProbeTaskProgress";
import { Select } from "./Select";
import { Textarea } from "./Textarea";
import { ModuleHeader } from "./WorkspaceHeader";

export interface CodexProbeFormState {
  questionId: string | null;
  customQuestion: string;
  customAnswer: string;
  customGradingVersion: CodexProbeGradingVersion;
  onCustomQuestionChange: (value: string) => void;
  onCustomAnswerChange: (value: string) => void;
  onCustomGradingChange: (value: CodexProbeGradingVersion) => void;
}

export interface CodexProbeSetupProps {
  active: boolean;
  probe: ReturnType<typeof useCodexProbe>;
  form: CodexProbeFormState;
  onQuestionChange: (id: string | null) => void;
  runCount: number;
  onRunCountChange: (count: number) => void;
}

export function CodexProbeSetup({ active, probe, form, onQuestionChange, runCount, onRunCountChange }: CodexProbeSetupProps) {
  const { t } = useI18n();
  const id = useId();
  const custom = form.questionId === CUSTOM_PROBE_QUESTION_ID;
  const selected = probe.questions?.find((question) => question.id === form.questionId);
  const maxRuns = custom ? probe.customMaxRuns : selected?.maxRuns ?? null;
  const effectiveRuns = maxRuns === null ? runCount : Math.min(runCount, maxRuns);
  const integer = form.customGradingVersion === "1";
  const ready = probe.synchronized && maxRuns !== null && probe.customTimeoutSeconds !== null
    && (!custom || (form.customQuestion.trim().length > 0 && (!integer || /^\d+$/.test(form.customAnswer.trim()))));
  const locked = !active || probe.running || probe.starting || Boolean(probe.status?.persistPending);
  const start = () => {
    if (!form.questionId || locked || !ready) return;
    void probe.start({ runCount: effectiveRuns, questionId: form.questionId,
      customQuestion: custom ? form.customQuestion.trim() : undefined,
      customAnswer: custom && integer ? form.customAnswer.trim() : undefined,
      customGradingVersion: custom ? form.customGradingVersion : undefined });
  };
  return <section className="asb-usage-surface" aria-labelledby={`${id}-setup`}>
    <ModuleHeader id={`${id}-setup`} title={t("usage.radar.setup")} />
    {probe.catalogError && <div className="asb-probe-state-row" role="alert">
      <p className="asb-warn-text">{t("codex.probe.catalogError", { error: probe.catalogError })}</p>
      <Button variant="secondary" size="compact" onClick={probe.refresh}>{t("codex.probe.retryRead")}</Button>
    </div>}
    {probe.startError && <p className="asb-warn-text" role="alert">{t("codex.probe.startError", { error: probe.startError })}</p>}
    <div className="asb-probe-setup-row">
      <div className="asb-probe-field asb-probe-question-picker">
        <span>{t("usage.radar.questionAria")}</span>
        <Select value={form.questionId} options={[
          ...(probe.questions ?? []).map((question) => ({ value: question.id, label: question.label })),
          { value: CUSTOM_PROBE_QUESTION_ID, label: t("usage.radar.customQuestion") },
        ]} placeholder={t(probe.questions === null ? "usage.radar.questionsLoading" : "usage.radar.selectQuestion")}
          ariaLabel={t("usage.radar.questionAria")} disabled={locked} onChange={onQuestionChange} />
      </div>
      {maxRuns !== null && <div className="asb-probe-field asb-probe-run-count">
        <label htmlFor={`${id}-runs`}>{t("usage.radar.runCountAria")}</label>
        <Input id={`${id}-runs`} type="number" min={1} max={maxRuns} step={1} value={effectiveRuns}
          disabled={locked || maxRuns === 1} onChange={(event) => {
            const next = Number.parseInt(event.target.value, 10);
            if (!Number.isNaN(next)) onRunCountChange(Math.min(maxRuns, Math.max(1, next)));
          }} />
      </div>}
      {probe.running ? <Button variant="secondary" disabled={!active || probe.cancelling || probe.starting} onClick={() => void probe.cancel()}>
        {t(probe.cancelling ? "usage.radar.cancelling" : "usage.radar.cancel")}
      </Button> : <Button variant="primary" disabled={locked || !ready || !form.questionId} onClick={start}>
        {t(probe.starting ? "usage.radar.starting" : "usage.radar.start")}
      </Button>}
    </div>
    <p className="asb-codex-probe-note">{t("usage.radar.cost")}</p>
    {custom && <CustomQuestionForm id={id} form={form} disabled={locked}
      timeoutSeconds={probe.customTimeoutSeconds?.[form.customGradingVersion]} />}
  </section>;
}

function CustomQuestionForm({ id, form, disabled, timeoutSeconds }: {
  id: string; form: CodexProbeFormState; disabled: boolean; timeoutSeconds: number | undefined;
}) {
  const { t } = useI18n();
  const integer = form.customGradingVersion === "1";
  return <div className="asb-probe-custom-fields">
    <div className="asb-probe-field asb-probe-custom-format">
      <span>{t("codex.probe.visual.answerFormat")}</span>
      <Select value={form.customGradingVersion} disabled={disabled}
        ariaLabel={t("codex.probe.visual.answerFormat")} options={[
          { value: "1", label: t("codex.probe.visual.integerFormat") },
          { value: "svg-visual-v1", label: t("codex.probe.visual.svgFormat") },
          { value: "html-visual-v1", label: t("codex.probe.visual.htmlFormat") },
        ]} onChange={(value) => form.onCustomGradingChange(value as CodexProbeGradingVersion)} />
    </div>
    {timeoutSeconds !== undefined && <p className="asb-codex-probe-meta">
      {t("codex.probe.timeLimit", { time: probeTime(timeoutSeconds) })}
    </p>}
    <div className="asb-probe-field">
      <label htmlFor={`${id}-question`}>{t("codex.probe.customQuestion")}</label>
      <Textarea id={`${id}-question`} rows={3} value={form.customQuestion}
        placeholder={t(integer ? "codex.probe.customQuestionPlaceholder" : "codex.probe.visual.questionPlaceholder")}
        disabled={disabled} onChange={(event) => form.onCustomQuestionChange(event.target.value)} />
    </div>
    {integer ? <div className="asb-probe-field">
      <label htmlFor={`${id}-answer`}>{t("codex.probe.expectedAnswer")}</label>
      <Input id={`${id}-answer`} placeholder={t("codex.probe.exampleAnswer")} value={form.customAnswer}
        disabled={disabled} onChange={(event) => form.onCustomAnswerChange(event.target.value)} />
    </div> : <p className="asb-codex-probe-note">{t("codex.probe.visual.note")}</p>}
  </div>;
}
