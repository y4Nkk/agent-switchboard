import { commandErrorText, errorText } from "../i18n/errors";
import { ConfirmSheet } from "../components/ConfirmSheet";
import { DiffView } from "../components/DiffView";
import { PreviewInspector } from "../components/PreviewInspector";
import { Time } from "../components/Time";
import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";
import type { useProviders } from "./useProviders";
import type { useSwitchOperations } from "./useSwitchOperations";

interface OperationConfirmSheetsProps {
  busy: boolean;
  operations: ReturnType<typeof useSwitchOperations>;
  providers: ReturnType<typeof useProviders>;
}

function ProviderSaveConfirmation({ providers, busy }: Pick<OperationConfirmSheetsProps, "providers" | "busy">) {
  const { t } = useI18n();
  const pending = providers.pendingSave;
  if (!pending) return null;
  return <ConfirmSheet title={t("operations.save.title")} busy={busy} confirmLabel={t("operations.save.title")}
    onConfirm={() => void providers.runPendingSave()} onCancel={() => void providers.cancelPendingSave()}>
    {providers.pendingSaveError != null && <p role="alert" className="asb-fail-text">{errorText(providers.pendingSaveError, t)}</p>}
    <p>{t("operations.save.writesTarget", { target: pending.preview.preview.target })}</p>
    <PreviewInspector filePreview={pending.preview} userConfigModel={null} userConfigWarnings={[]} />
  </ConfirmSheet>;
}

/** Every destructive or writing operation gets one explicit confirmation
 * sheet; nothing writes without it. The switch confirmation is not a sheet:
 * it unfolds inline inside the requesting provider row. */
export function OperationConfirmSheets({
  operations,
  providers,
  busy,
}: OperationConfirmSheetsProps) {
  const { t } = useI18n();
  const { undoPending, recoverLockPending } = operations;
  return (
    <>
      <ProviderSaveConfirmation providers={providers} busy={busy} />
      {providers.deletePending && (
        <ConfirmSheet
          title={t("operations.delete.title")}
          busy={busy}
          confirmLabel={t("operations.delete.confirm")}
          destructive
          onConfirm={() => void providers.runDelete()}
          onCancel={() => providers.setDeletePending(null)}
        >
          <ul className="asb-dialog-details">
            <li>{t("operations.delete.line1", { name: providers.deletePending.kind === "generic"
              ? providers.deletePending.profile.name
              : providers.deletePending.record.profile.name })}</li>
            <li>{t("operations.delete.line2")}</li>
          </ul>
        </ConfirmSheet>
      )}
      {undoPending && (
        <ConfirmSheet
          title={t("operations.undo.title")}
          busy={busy}
          confirmLabel={t("operations.undo.confirm")}
          confirmDisabled={operations.undoDiff.state !== "ready"}
          onConfirm={() => void operations.runUndo()}
          onCancel={operations.cancelUndo}
        >
          <ul className="asb-dialog-details">
            <li>{undoPending.profileName
              ? t("operations.undo.lastSwitch", { client: clientName(undoPending.app), name: undoPending.profileName })
              : t("operations.undo.lastRestore", { client: clientName(undoPending.app) })}</li>
            <li>{t("operations.undo.switchedAt")} <Time iso={undoPending.at} /></li>
            <li>{t("operations.undo.restoreNote")}</li>
            {operations.undoDiff.state === "loading" ? (
              <li>{t("operations.undo.diffLoading")}</li>
            ) : operations.undoDiff.state === "error" ? (
              <li className="asb-warn-text">{commandErrorText(operations.undoDiff.error, t)}</li>
            ) : operations.undoDiff.state === "ready" && operations.undoDiff.changes.length === 0 ? (
              <li>{t("operations.undo.diffEmpty")}</li>
            ) : operations.undoDiff.state === "ready" ? (
              <li><DiffView changes={operations.undoDiff.changes.map((change) => ({
                ...change,
                before: change.after,
                after: change.before,
              }))} label={t("operations.undo.diffLabel")} /></li>
            ) : null}
          </ul>
        </ConfirmSheet>
      )}
      {recoverLockPending && (
        <ConfirmSheet
          title={t("operations.recoverLock.title")}
          busy={busy}
          confirmLabel={t("operations.recoverLock.confirm")}
          destructive
          onConfirm={() => void operations.runRecoverStaleLock()}
          onCancel={() => operations.setRecoverLockPending(null)}
        >
          <ul className="asb-dialog-details">
            <li>{t("operations.recoverLock.line1", { client: clientName(recoverLockPending) })}</li>
            <li>{t("operations.recoverLock.line2")}</li>
          </ul>
        </ConfirmSheet>
      )}
    </>
  );
}
