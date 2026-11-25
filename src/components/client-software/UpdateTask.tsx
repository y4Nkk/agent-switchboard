import { useId, useState, type ReactNode } from "react";
import type { ClientUpdatePhase, ClientUpdateTask } from "../../api/client-software";
import { useI18n, type MessageKey, type TFunction } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { CodePreview } from "../CodePreview";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { FactList, type FactListRow } from "../FactList";
import { CheckIcon, DashIcon, UpdateFailureIcon } from "../icons";
import { Time } from "../Time";
import { ToolTrack, type ToolStation, type ToolStationState } from "../client-management/ToolTrack";

const STEPS: MessageKey[] = ["software.step.prepare", "software.step.install", "software.step.verify"];

/** The phases shown as a panel: the operation is running or stopped at an
 * error. A completed or cancelled operation is only a receipt. */
type PanelPhase = Exclude<ClientUpdatePhase, "completed" | "cancelled">;

/** Which steps the operation finished, is in or stopped at. Only the backend's
 * phase decides; the rail never estimates progress. */
const STEP_STATES: Record<PanelPhase, ToolStationState[]> = {
  preparing: ["current", "pending", "pending"],
  runningUpdater: ["done", "current", "pending"],
  verifying: ["done", "done", "current"],
  failed: ["failed", "pending", "pending"],
  needsAttention: ["done", "done", "attention"],
};

const TONES: Record<ClientUpdatePhase, "update" | "current" | "warning" | "neutral"> = {
  preparing: "update", runningUpdater: "update", verifying: "update", completed: "current",
  failed: "warning", needsAttention: "warning", cancelled: "neutral",
};

function phaseStations(phase: PanelPhase, t: TFunction): ToolStation[] {
  const states = STEP_STATES[phase];
  return states.map((state, index): ToolStation => ({
    label: t(STEPS[index]), state,
    link: index < states.length - 1 && state === "done" ? "done" : undefined,
  }));
}

function appName(task: ClientUpdateTask): string {
  return task.plan.app === "codex" ? "Codex" : "Claude Code";
}

/** A finished operation in one sentence: the client, how it is installed and,
 * once completed, from which version to the one that was verified. */
function receiptText(task: ClientUpdateTask, t: TFunction): ReactNode {
  const identity = { app: appName(task), manager: t(`software.manager.${task.plan.manager}`) };
  if (task.phase !== "completed") return t("software.receipt.cancelled", identity);
  return <>
    {t("software.receipt.updated", identity)}
    {task.actualVersion && <> <code>{task.plan.fromVersion} → {task.actualVersion}</code></>}
  </>;
}

export function UpdateTask({ task, busy, onCancel }: {
  task: ClientUpdateTask; busy: boolean; onCancel: () => void;
}) {
  const { t } = useI18n();
  const [output, setOutput] = useState<boolean | null>(null);
  const outputId = useId();
  const phase = task.phase;
  const finished = phase === "completed" || phase === "cancelled";
  const failed = phase === "failed" || phase === "needsAttention";
  const outputOpen = output ?? failed;
  const hasOutput = task.log.length > 0;
  const rows: FactListRow[] = [
    { label: t("software.target"), value: <code>{task.plan.fromVersion} → {task.plan.targetVersion}</code> },
    { label: t("software.launcher"), value: <code>{task.plan.launcher}</code> },
    ...(task.actualVersion ? [{ label: t("software.actual"), value: <code>{task.actualVersion}</code> }] : []),
  ];
  return <section className={finished ? "asb-software-task" : "asb-tool-stage asb-software-task"}
    data-finished={finished || undefined} data-tone={TONES[phase]} aria-label={t("software.task")}>
    <div className="asb-software-task-head">
      <p className="asb-tool-status" role="status">
        <span className="asb-software-task-mark" aria-hidden="true">
          {phase === "completed" ? <CheckIcon /> : phase === "cancelled" ? <DashIcon />
            : failed ? <UpdateFailureIcon size={14} /> : <span className="asb-tool-lamp" />}
        </span>
        <span>{finished ? receiptText(task, t) : `${appName(task)} · ${t(`software.phase.${phase}`)}`}</span>
      </p>
      {finished
        ? task.finishedAt && <Time iso={task.finishedAt} />
        : <span className="asb-software-task-start">{t("software.startedAt")} <Time iso={task.startedAt} /></span>}
      {task.cancellable && <Button variant="secondary" size="compact" disabled={busy}
        onClick={onCancel}>{t("software.cancel")}</Button>}
    </div>
    {!finished && <>
      <ToolTrack fit stations={phaseStations(phase, t)} />
      <FactList rows={rows} />
      {task.error && <p role="alert" className="asb-warn-text">{localizedMessageText(task.error, t)}</p>}
    </>}
    {hasOutput && <CollapsibleTrigger className="asb-software-task-output"
      open={outputOpen} contentId={outputId} label={t("software.output")} onOpenChange={setOutput} />}
    {hasOutput && <Collapsible open={outputOpen} id={outputId}>
      <CodePreview target={t("software.output")} content={task.log.join("\n")} />
    </Collapsible>}
  </section>;
}
