import { useState } from "react";
import type { ClientUpdateTask } from "../../api/client-software";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { CodePreview } from "../CodePreview";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { Time } from "../Time";

export function UpdateTask({ task, busy, onCancel }: {
  task: ClientUpdateTask; busy: boolean; onCancel: () => void;
}) {
  const { t } = useI18n();
  const [output, setOutput] = useState<boolean | null>(null);
  const failed = task.phase === "failed" || task.phase === "needsAttention";
  return <section className="asb-software-task" aria-label={t("software.task")}>
    <div className="asb-software-heading">
      <strong>{task.plan.app === "codex" ? "Codex" : "Claude Code"}</strong>
      <span role="status">{t(`software.phase.${task.phase}`)}</span>
      <Time iso={task.startedAt} />
    </div>
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
