import { useState } from "react";
import type { ClientUpdatePhase, ClientUpdateTask } from "../../api/client-software";
import { useI18n, type MessageKey, type TFunction } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { CodePreview } from "../CodePreview";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { Time } from "../Time";
import { ToolTrack, type ToolStation, type ToolStationState } from "../client-management/ToolTrack";

const STEPS: MessageKey[] = ["software.step.prepare", "software.step.install", "software.step.verify"];

/** Which steps the task finished, is in or stopped at. Only the backend's
 * phase decides; the rail never estimates progress. */
const STEP_STATES: Record<ClientUpdatePhase, ToolStationState[]> = {
  preparing: ["current", "pending", "pending"],
  runningUpdater: ["done", "current", "pending"],
  verifying: ["done", "done", "current"],
  completed: ["done", "done", "done"],
  failed: ["failed", "pending", "pending"],
  needsAttention: ["done", "done", "attention"],
  cancelled: ["cancelled", "pending", "pending"],
};

const STEP_TONES: Record<ClientUpdatePhase, "update" | "current" | "warning" | "neutral"> = {
  preparing: "update", runningUpdater: "update", verifying: "update", completed: "current",
  failed: "warning", needsAttention: "warning", cancelled: "neutral",
};

function phaseStations(phase: ClientUpdatePhase, t: TFunction): ToolStation[] {
  const states = STEP_STATES[phase];
  return states.map((state, index): ToolStation => ({
    label: t(STEPS[index]), state,
    link: index < states.length - 1 && state === "done" ? "done" : undefined,
  }));
}

export function UpdateTask({ task, busy, onCancel }: {
  task: ClientUpdateTask; busy: boolean; onCancel: () => void;
}) {
  const { t } = useI18n();
  const [output, setOutput] = useState<boolean | null>(null);
  const failed = task.phase === "failed" || task.phase === "needsAttention";
  return <section className="asb-tool-card asb-software-task" data-tone={STEP_TONES[task.phase]} aria-label={t("software.task")}>
    <div className="asb-software-task-head">
      <strong>{task.plan.app === "codex" ? "Codex" : "Claude Code"}</strong>
      <p className="asb-tool-status" role="status">
        <span className="asb-tool-lamp" aria-hidden="true" />{t(`software.phase.${task.phase}`)}
      </p>
      <Time iso={task.startedAt} />
    </div>
    <ToolTrack stations={phaseStations(task.phase, t)} />
    <FactList rows={[
      { label: t("software.launcher"), value: <code>{task.plan.launcher}</code> },
      { label: t("software.target"), value: `${task.plan.fromVersion} → ${task.plan.targetVersion}` },
      ...(task.actualVersion ? [{ label: t("software.actual"), value: task.actualVersion }] : []),
    ]} />
    {task.error && <p role="alert" className="asb-warn-text">{localizedMessageText(task.error, t)}</p>}
    {task.cancellable && <Button variant="secondary" size="compact" disabled={busy} onClick={onCancel}>{t("software.cancel")}</Button>}
    {task.log.length > 0 && <CollapsibleSection quiet open={output ?? failed} onOpenChange={setOutput} label={t("software.output")}>
      <CodePreview target={t("software.output")} content={task.log.join("\n")} />
    </CollapsibleSection>}
  </section>;
}
