import type { AppKind, LocalizedMessage } from "../api/client";
import { useI18n } from "../i18n";
import { errorDiagnosticText, errorText, localizedMessageText } from "../i18n/errors";
import { toast, toastMessage, type ToastContent } from "../components/use-toast";
import { ToastText } from "../components/ToastContent";
import { clientName } from "../lib/client-name";

/** Renders one structured warning row in the current language.
 * Used inside the toaster so visible warnings follow a language switch. */
export function ToastWarningRows({ warnings }: { warnings: readonly LocalizedMessage[] }) {
  const { t } = useI18n();
  return (
    <>
      {warnings.map((warning, index) => (
        <div key={`${index}:${warning.key}`}>
          {localizedMessageText(warning, t)}
        </div>
      ))}
    </>
  );
}

/** Renders the app-owned explanation of one command error, keeping the
 * scrubbed backend message as the diagnostic detail line. */
export function CommandErrorLines({ error }: { error: unknown }) {
  const { t } = useI18n();
  const diagnostic = errorDiagnosticText(error, t);
  const message = errorText(error, t);
  return (
    <>
      <div>{message}</div>
      {diagnostic ? <div className="asb-scope-note">{diagnostic}</div> : null}
    </>
  );
}

/** Warning-toast title: the caller's title plus the live warning count, both
 * resolved in the current language at render time. */
function ToastWarningTitle({ title, count }: { title: ToastContent; count: number }) {
  const { t } = useI18n();
  return (
    <>
      <ToastText content={title} />
      {t("operations.notify.warningsSuffix", { count })}
    </>
  );
}

/** Warning list shared by every write path: one warning toast, each line its
 * own row (DESIGN.md §8 全局通知). The rows translate at render time, so a
 * visible toast follows a language switch. */
export function notifyWarnings(title: ToastContent, warnings: readonly LocalizedMessage[]) {
  toast({
    kind: "warning",
    title: <ToastWarningTitle title={title} count={warnings.length} />,
    description: <ToastWarningRows warnings={warnings} />,
  });
}

/** One-shot write results (switch / restore / undo) report through the global
 * toaster; the banner layer stays reserved for persistent errors that carry
 * a decision (DESIGN.md §7). */
export function notifyWriteOutcome(title: ToastContent, app: AppKind, warnings: readonly LocalizedMessage[]) {
  if (warnings.length > 0) {
    notifyWarnings(title, warnings);
    return;
  }
  toast({
    kind: "success",
    title,
    description: toastMessage("operations.notify.readAtLaunch", { client: clientName(app) }),
  });
}
