import type { ApplicationUpdateControl } from "../app/update-state";
import { CommandErrorLines } from "../app/notifications";
import { useI18n } from "../i18n";
import { ConfirmSheet } from "./ConfirmSheet";

export function UpdateRestartDialog({ update }: { update: ApplicationUpdateControl }) {
  const { t } = useI18n();
  if (!update.restartConfirmationOpen) return null;
  return <ConfirmSheet title={t("backup.update.restartTitle")} busy={update.state.restarting}
    confirmLabel={t("backup.update.restartNow")} cancelLabel={t("backup.update.restartLater")}
    onConfirm={update.onConfirmRestart} onCancel={update.onDismissRestart}>
    <p>{t("backup.update.restartPrompt")}</p>
    {update.state.restartError != null && <div className="asb-fail-text" role="alert">
      <CommandErrorLines error={update.state.restartError} />
    </div>}
  </ConfirmSheet>;
}
