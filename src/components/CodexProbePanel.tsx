import { useMemo } from "react";
import { CUSTOM_PROBE_QUESTION_ID, type CodexProbeBatch, type CodexProbeGradingVersion } from "../api/client";
import type { TFunction } from "../i18n";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { createProbeHtmlPreview } from "../lib/probe-html";
import { Input } from "./Input";
import { Select } from "./Select";
import { ProbeTaskProgress, probeTime } from "./ProbeTaskProgress";
import {
  ProbeRunsTable, configSummary, probeSummaryCards, summarizeRuns,
} from "./probe-shared";
import { Textarea } from "./Textarea";
import { Time } from "./Time";
import { ModuleHeader } from "./WorkspaceHeader";
import { UsageIcon } from "./icons";
import { StatCards } from "./charts/stat-cards";
import type { useCodexProbe } from "../app/useCodexProbe";

/** The custom-question form lives with the panel because only a `custom`
 * selection needs it; the header keeps the action and the pickers. */
export interface CodexProbeFormState {
  questionId: string | null;
  customQuestion: string;
  customAnswer: string;
  customGradingVersion: CodexProbeGradingVersion;
  onCustomQuestionChange: (value: string) => void;
  onCustomAnswerChange: (value: string) => void;
  onCustomGradingChange: (value: CodexProbeGradingVersion) => void;
}

function resultLine(batch: CodexProbeBatch, t: TFunction): string | null {
  const summary = summarizeRuns(batch.runs);
  switch (batch.status) {
    case "running":
      return null;
    case "completed":
      return batch.gradingVersion !== "1" ? t("codex.probe.visual.completed")
        : t("codex.probe.completedLine", { passed: summary.passedCount, judged: summary.judgedCount });
    case "cancelled":
      return t("codex.probe.cancelledLine");
    case "failed":
      return t("codex.probe.failedLine");
    case "interrupted":
      return t("codex.probe.interruptedLine");
    case "config-changed":
      return t("codex.probe.configChangedLine");
  }
}

function CustomQuestionForm({ form, disabled, timeoutSeconds }: {
  form: CodexProbeFormState; disabled: boolean; timeoutSeconds: number | undefined;
}) {
  const { t } = useI18n();
  if (form.questionId !== CUSTOM_PROBE_QUESTION_ID) return null;
  return (
    <div className="asb-codex-probe-question">
      <span>{t("codex.probe.visual.answerFormat")}</span>
      <Select value={form.customGradingVersion} disabled={disabled}
        ariaLabel={t("codex.probe.visual.answerFormat")} options={[
          { value: "1", label: t("codex.probe.visual.integerFormat") },
          { value: "svg-visual-v1", label: t("codex.probe.visual.svgFormat") },
          { value: "html-visual-v1", label: t("codex.probe.visual.htmlFormat") },
        ]} onChange={(value) => form.onCustomGradingChange(value as CodexProbeGradingVersion)} />
      {timeoutSeconds !== undefined && <p className="asb-codex-probe-meta">
        {t("codex.probe.timeLimit", { time: probeTime(timeoutSeconds) })}
      </p>}
      <label htmlFor="codex-probe-custom-question">{t("codex.probe.customQuestion")}</label>
      <Textarea id="codex-probe-custom-question" rows={3} value={form.customQuestion}
        placeholder={t(form.customGradingVersion !== "1"
          ? "codex.probe.visual.questionPlaceholder" : "codex.probe.customQuestionPlaceholder")}
        disabled={disabled} onChange={(event) => form.onCustomQuestionChange(event.target.value)} />
      {form.customGradingVersion === "1" ? <>
        <label htmlFor="codex-probe-custom-answer">{t("codex.probe.expectedAnswer")}</label>
        <Input id="codex-probe-custom-answer" placeholder={t("codex.probe.exampleAnswer")} value={form.customAnswer}
          disabled={disabled} onChange={(event) => form.onCustomAnswerChange(event.target.value)} />
      </> : <p className="asb-codex-probe-note">{t("codex.probe.visual.note")}</p>}
    </div>
  );
}

/** The catalog's hand-made reference rendering through the same preview
 * pipeline as real answers, always in view; one possible output only. */
function ReferenceVisual({ answer }: { answer: string }) {
  const { t } = useI18n();
  const source = useMemo(() => createProbeHtmlPreview(answer), [answer]);
  if (!source) return null;
  return (
    <>
      <p className="asb-codex-probe-meta">{t("codex.probe.visual.referenceTitle")}</p>
      <iframe className="asb-probe-html" srcDoc={source} sandbox="allow-scripts"
        referrerPolicy="no-referrer" loading="lazy"
        title={t("codex.probe.visual.referenceTitle")} />
    </>
  );
}

function ProbeResults({ status, probe }: {
  status: CodexProbeBatch;
  probe: ReturnType<typeof useCodexProbe>;
}) {
  const { t } = useI18n();
  const summary = summarizeRuns(status.runs);
  const heading = status.status === "running" ? t("codex.probe.currentHeading") : t("codex.probe.lastHeading");
  return (
    <div className="asb-codex-probe-content">
      <p className="asb-codex-probe-meta">
        {heading} · {t("codex.probe.metaQuestion")}{status.question.label || "—"} · {t("codex.probe.startedAt")} <Time iso={status.startedAt} />
        {status.finishedAt && <> · {t("codex.probe.finishedAt")} <Time iso={status.finishedAt} /></>}
      </p>
      <p className="asb-codex-probe-meta">
        {t("codex.probe.configPrefix")}{configSummary(status, t)}
        {status.cliVersion && ` · CLI ${status.cliVersion}`}
      </p>
      {status.status !== "running" && <p className="asb-codex-probe-progress" role="status" aria-live="polite">
        {resultLine(status, t)}
      </p>}
      {status.status !== "running" && status.statusError &&
        <p className="asb-warn-text" role="alert">{status.statusError}</p>}
      {status.persistPending && <p className="asb-warn-text" role="alert">
        {status.persistError ?? t("codex.probe.persistPending")}
        {" "}
        <Button variant="secondary" disabled={probe.retrying}
          onClick={() => void probe.retrySave()}>
          {probe.retrying ? t("codex.probe.retryingSave") : t("codex.probe.retrySave")}
        </Button>
      </p>}
      {summary.runCount > 0 && <>
        <div role="group" aria-label={t("codex.probe.summaryAria")}><StatCards stats={probeSummaryCards(status, t)} /></div>
        {summary.recordedRuns < summary.runCount && <p className="asb-codex-probe-note" role="status">
          {t("codex.probe.partialUsage", { recorded: summary.recordedRuns, total: summary.runCount })}
        </p>}
        <ProbeRunsTable batch={status} />
      </>}
      {(status.status === "cancelled" || status.status === "interrupted") && <p className="asb-codex-probe-note">
        {t("codex.probe.cancelledNote")}
      </p>}
    </div>
  );
}

export function CodexProbePanel({ probe, form }: {
  probe: ReturnType<typeof useCodexProbe>;
  form: CodexProbeFormState;
}) {
  const { t } = useI18n();
  const selected = probe.questions?.find((question) => question.id === form.questionId);
  return (
    <section className="asb-panel asb-codex-probe" aria-label={t("codex.probe.title")}>
      <ModuleHeader title={t("codex.probe.title")} />
      <p className="asb-codex-probe-note">{t("codex.probe.backgroundNote")}</p>
      {probe.catalogError && <p className="asb-warn-text" role="alert">
        {t("codex.probe.catalogError", { error: probe.catalogError })}{" "}
        <Button variant="secondary" onClick={probe.refresh}>{t("codex.probe.retryRead")}</Button>
      </p>}
      {probe.startError && <p className="asb-warn-text" role="alert">{t("codex.probe.startError", { error: probe.startError })}</p>}
      {probe.readError && <p className="asb-warn-text" role="alert">
        {t("codex.probe.readError", { error: probe.readError })}{" "}
        <Button variant="secondary" onClick={probe.refresh}>
          {t("codex.probe.retryRead")}
        </Button>
      </p>}
      {probe.cancelError && <p className="asb-warn-text" role="alert">{t("codex.probe.cancelError", { error: probe.cancelError })}</p>}
      {probe.retryError && <p className="asb-warn-text" role="alert">{t("codex.probe.retryError", { error: probe.retryError })}</p>}
      {probe.starting ? <p className="asb-codex-probe-progress" role="status">{t("codex.probe.preparing")}</p>
        : !probe.synchronized && !probe.readError
          ? <p className="asb-codex-probe-progress" role="status">{t("codex.probe.readingProgress")}</p>
          : probe.status && <ProbeTaskProgress batch={probe.status} stale={Boolean(probe.readError)} />}
      <CustomQuestionForm form={form} disabled={probe.running || probe.starting}
        timeoutSeconds={probe.customTimeoutSeconds?.[form.customGradingVersion]} />
      {selected && <div className="asb-codex-probe-question">
        <p className="asb-codex-probe-meta">
          {t("codex.probe.metaQuestion")}{selected.label} · {t("codex.probe.timeLimit", { time: probeTime(selected.timeoutSeconds) })}
        </p>
        <p className="asb-probe-question-text">{selected.text}</p>
        {selected.referenceVisual && <ReferenceVisual answer={selected.referenceVisual} />}
        {selected.gradingVersion !== "1" &&
          <p className="asb-codex-probe-note">{t("codex.probe.visual.note")}</p>}
      </div>}
      {probe.status ? <ProbeResults status={probe.status} probe={probe} /> : (
        <div className="asb-empty-state asb-codex-probe-empty">
          <span className="asb-empty-state-icon" aria-hidden="true"><UsageIcon /></span>
          <h3 className="asb-section-title" role="status">
            {probe.starting ? t("codex.probe.starting") : !probe.synchronized ? t("codex.probe.readingProgress")
              : t("codex.probe.notStarted")}
          </h3>
        </div>
      )}
      <p className="asb-codex-probe-note">
        {t("codex.probe.note")}
      </p>
    </section>
  );
}
