import type { CodexProbeBatch, CodexProbeRunPhase } from "../api/client";
import { useI18n, type MessageKey } from "../i18n";

const PHASE_LABEL: Record<CodexProbeRunPhase, MessageKey> = {
  starting: "codex.probe.phase.starting",
  running: "codex.probe.phase.running",
  collecting: "codex.probe.phase.collecting",
  saving: "codex.probe.phase.saving",
  cancelling: "codex.probe.phase.cancelling",
};

export function probeTime(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return Math.floor(whole / 60).toString().padStart(2, "0") + ":" + (whole % 60).toString().padStart(2, "0");
}

/** Time is the last backend reading; only completed runs measure task progress. */
export function ProbeTaskProgress({ batch, stale = false }: { batch: CodexProbeBatch; stale?: boolean }) {
  const { t } = useI18n();
  if (batch.status !== "running" || batch.persistPending) return null;
  const progress = batch.progress;
  const clock = progress.kind === "run" ? progress : null;
  const executing = clock?.phase === "starting" || clock?.phase === "running";
  const remaining = clock ? Math.max(0, Math.ceil((clock.timeLimitMs - clock.elapsedMs) / 1000)) : 0;
  const elapsed = clock ? Math.floor(clock.elapsedMs / 1000) : 0;
  const phase = progress.kind === "run"
    ? executing && remaining === 0 ? t("codex.probe.limitReached") : t(PHASE_LABEL[progress.phase])
    : t(progress.cancelling ? "codex.probe.phase.cancelling" : "codex.probe.preparing");
  return <div className="asb-probe-task" role="group" aria-label={t("codex.probe.taskProgress")}>
    <div className="asb-probe-task-top">
      <div>
        <p className="asb-codex-probe-meta">
          {batch.question.label} · {t("codex.probe.runPosition", {
            current: progress.seq, total: batch.plannedRuns,
          })}
        </p>
        <p className="asb-probe-task-phase" role="status" aria-live="polite">{phase}</p>
      </div>
      {clock && <dl className="asb-probe-task-clock" aria-live="off">
        <dt>{t(executing ? "codex.probe.remaining" : "codex.probe.elapsedLabel")}</dt>
        <dd>{probeTime(executing ? remaining : elapsed)}</dd>
      </dl>}
    </div>
    {clock && <p className="asb-codex-probe-meta">
      {t("codex.probe.elapsed", { time: probeTime(elapsed) })} · {t("codex.probe.timeLimit", {
        time: probeTime(clock.timeLimitMs / 1000),
      })}
    </p>}
    <div className="asb-probe-task-completion">
      <p className="asb-codex-probe-meta">{t("codex.probe.completedRuns", {
        done: batch.completedRuns, total: batch.plannedRuns,
      })}</p>
      <progress className="asb-probe-task-meter" max={batch.plannedRuns} value={batch.completedRuns}
        aria-label={t("codex.probe.taskProgress")}
        aria-valuetext={t("codex.probe.completedRuns", { done: batch.completedRuns, total: batch.plannedRuns })} />
    </div>
    {stale && <p className="asb-warn-text" role="status">{t("codex.probe.staleProgress")}</p>}
  </div>;
}
