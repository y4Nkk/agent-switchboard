import { useId, useMemo } from "react";
import type { CodexProbeBatch } from "../api/client";
import type { useCodexProbe } from "../app/useCodexProbe";
import { useI18n, type TFunction } from "../i18n";
import { createProbeHtmlPreview } from "../lib/probe-html";
import { Button } from "./Button";
import { CodexProbeSetup, type CodexProbeSetupProps } from "./CodexProbeSetup";
import { ProbeTaskProgress, probeTime } from "./ProbeTaskProgress";
import { ProbeRunsTable, configSummary, probeSummaryCards, summarizeRuns } from "./probe-shared";
import { Time } from "./Time";
import { ModuleHeader } from "./WorkspaceHeader";
import { StatCards } from "./charts/stat-cards";
import { useCodexProbeDetail } from "./use-codex-probe-detail";

type Probe = ReturnType<typeof useCodexProbe>;
type ProbeQuestion = NonNullable<Probe["questions"]>[number];

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

function ReferenceVisual({ answer }: { answer: string }) {
  const { t } = useI18n();
  const source = useMemo(() => createProbeHtmlPreview(answer), [answer]);
  if (!source) return null;
  const title = t("codex.probe.visual.referenceTitle");
  return <figure className="asb-probe-reference">
    <figcaption className="asb-codex-probe-meta">{title}</figcaption>
    <iframe className="asb-probe-html" srcDoc={source} sandbox="allow-scripts"
      referrerPolicy="no-referrer" loading="lazy" title={title} />
  </figure>;
}

function ProbeQuestionPanel({ question, active }: { question: ProbeQuestion; active: boolean }) {
  const { t } = useI18n();
  const heading = useId();
  const visual = active ? question.referenceVisual : null;
  return <section className="asb-usage-surface" aria-labelledby={heading}>
    <ModuleHeader id={heading} title={question.label} />
    <div className="asb-probe-question-layout" data-reference={Boolean(visual)}>
      <div className="asb-codex-probe-question">
        <p className="asb-codex-probe-meta">{t("codex.probe.timeLimit", { time: probeTime(question.timeoutSeconds) })}</p>
        <p className="asb-probe-question-text">{question.text}</p>
        {question.gradingVersion !== "1" && <p className="asb-codex-probe-note">{t("codex.probe.visual.note")}</p>}
      </div>
      {visual && <ReferenceVisual answer={visual} />}
    </div>
  </section>;
}

function ProbeProgressPanel({ probe }: { probe: Probe }) {
  const { t } = useI18n();
  const heading = useId();
  const batch = probe.status?.status === "running" && !probe.status.persistPending ? probe.status : null;
  if (!probe.starting && probe.synchronized && !batch && !probe.readError && !probe.cancelError) return null;
  return <section className="asb-usage-surface asb-probe-progress-surface" aria-labelledby={heading}>
    <ModuleHeader id={heading} title={t("codex.probe.taskProgress")} />
    {probe.readError && <div className="asb-probe-state-row" role="alert">
      <p className="asb-warn-text">{t("codex.probe.readError", { error: probe.readError })}</p>
      <Button variant="secondary" size="compact" onClick={probe.refresh}>{t("codex.probe.retryRead")}</Button>
    </div>}
    {probe.cancelError && <p className="asb-warn-text" role="alert">{t("codex.probe.cancelError", { error: probe.cancelError })}</p>}
    {probe.starting ? <p className="asb-codex-probe-progress" role="status">{t("codex.probe.preparing")}</p>
      : batch ? <ProbeTaskProgress batch={batch} stale={Boolean(probe.readError)} />
        : !probe.synchronized && !probe.readError && <p className="asb-codex-probe-progress" role="status">{t("codex.probe.readingProgress")}</p>}
    {batch && <p className="asb-codex-probe-note">{t("codex.probe.backgroundNote")}</p>}
  </section>;
}

function ProbeResults({ status, probe }: {
  status: CodexProbeBatch;
  probe: Probe;
}) {
  const { t } = useI18n();
  const summary = summarizeRuns(status.runs);
  return (
    <div className="asb-codex-probe-content">
      <p className="asb-codex-probe-meta">
        {t("codex.probe.metaQuestion")}{status.question.label || "—"} · {t("codex.probe.startedAt")} <Time iso={status.startedAt} />
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
        <ProbeRunsTable batch={status} />
      </>}
      {(status.status === "cancelled" || status.status === "interrupted") && <p className="asb-codex-probe-note">
        {t("codex.probe.cancelledNote")}
      </p>}
    </div>
  );
}

function ProbeResultPanel({ probe, detail }: { probe: Probe; detail: ReturnType<typeof useCodexProbeDetail> }) {
  const { t } = useI18n();
  const heading = useId();
  const batch = detail.detail;
  const awaiting = probe.status !== null && !detail.detailError && batch === null;
  const empty = !probe.status && !probe.starting && probe.synchronized;
  if (!batch && !awaiting && !empty && !detail.detailError && !probe.retryError) return null;
  const title = batch ? t(batch.status === "running" ? "codex.probe.currentHeading" : "codex.probe.lastHeading")
    : t("usage.radar.results");
  return <section className="asb-usage-surface" aria-labelledby={heading}>
    <ModuleHeader id={heading} title={title} />
    {detail.detailError && <div className="asb-probe-state-row" role="alert">
      <p className="asb-warn-text">{detail.detailError}</p>
      <Button variant="secondary" size="compact" onClick={detail.refreshDetail}>{t("codex.probe.retryRead")}</Button>
    </div>}
    {probe.retryError && <p className="asb-warn-text" role="alert">{t("codex.probe.retryError", { error: probe.retryError })}</p>}
    {awaiting && <p className="asb-codex-probe-meta" role="status">{t("codex.history.detailLoading")}</p>}
    {batch ? <ProbeResults status={batch} probe={probe} /> : empty &&
      <p className="asb-codex-probe-meta" role="status">{t("codex.probe.notStarted")}</p>}
  </section>;
}

export function CodexProbePanel(props: CodexProbeSetupProps) {
  const { t } = useI18n();
  const { probe, form, active } = props;
  const selected = probe.questions?.find((question) => question.id === form.questionId);
  const detail = useCodexProbeDetail(active, probe.status?.batchId ?? null, probe.detailRevision, false);
  return <section className="asb-usage-stack asb-codex-probe" aria-label={t("codex.probe.title")}>
    <CodexProbeSetup {...props} />
    {active && <ProbeProgressPanel probe={probe} />}
    {selected && <ProbeQuestionPanel question={selected} active={active} />}
    {active && <ProbeResultPanel probe={probe} detail={detail} />}
  </section>;
}
