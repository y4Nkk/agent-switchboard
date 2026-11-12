import { useEffect, useId, useRef, useState } from "react";
import { Dialog, Popover } from "react-aria-components";
import type { CommandError } from "../api/client";
import type { ApplicationPreparation } from "../api/application-preparation";
import { PreparationDetails } from "./ApplicationPreparation";
import { useI18n } from "../i18n";
import { commandErrorText, errorText } from "../i18n/errors";
import { Button } from "./Button";
import { CloseIcon, TrashIcon } from "./icons";
import { Tooltip } from "./Tooltip";
import { ToastStatusIcon } from "./ToastStatusIcon";
import { ToastText } from "./ToastContent";
import { ScrollArea } from "./ScrollArea";
import { useToast, type ToastContent, type ToastKind } from "./use-toast";

interface Props {
  error: CommandError | null;
  settingsError: CommandError | null;
  busy: boolean;
  preparation: ApplicationPreparation;
}

interface InformationMessage {
  id: string;
  kind: ToastKind;
  title: ToastContent;
  description?: ToastContent;
  onDismiss: () => void;
  toastId?: string;
}

function InformationRow({ message, onBeforeDismiss }: {
  message: InformationMessage;
  onBeforeDismiss: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="asb-information-row">
      <ToastStatusIcon kind={message.kind} />
      <div className="asb-information-copy">
        <div className="asb-information-title"><ToastText content={message.title} /></div>
        {message.description && (
          <div className="asb-information-description"><ToastText content={message.description} /></div>
        )}
      </div>
      <Tooltip label={t("shell.information.delete")} className="asb-information-tooltip">
        <Button variant="icon" aria-label={t("shell.information.delete")} onClick={() => {
          onBeforeDismiss();
          message.onDismiss();
        }}>
          <TrashIcon />
        </Button>
      </Tooltip>
    </div>
  );
}

/** Acknowledgement belongs to this source occurrence, never to its translated text. */
function useInformationDismissal(source: unknown) {
  const [notice, setNotice] = useState({ source, dismissed: false });
  if (notice.source !== source) setNotice({ source, dismissed: false });
  return {
    hidden: notice.source === source && notice.dismissed,
    dismiss: () => setNotice({ source, dismissed: true }),
  };
}

function useInformationMessages({ error, settingsError, busy, preparation }: Props) {
  const { t } = useI18n();
  const queue = useToast();
  const settings = useInformationDismissal(settingsError);
  const operation = useInformationDismissal(error);
  const preparationId = preparation.status !== "running"
    && (preparation.issues.length > 0 || preparation.repairedFiles.length > 0)
    ? `preparation-${preparation.sessionId}-${preparation.runId}-${preparation.revision}` : null;
  const prepared = useInformationDismissal(preparationId);
  const processing = useInformationDismissal(busy);
  const messages: InformationMessage[] = [];

  if (settingsError && !settings.hidden) messages.push({
    id: "settings-error", kind: "error",
    title: t("shell.banner.settingsUnavailable", { detail: errorText(settingsError, t) }),
    onDismiss: settings.dismiss,
  });
  if (error && !operation.hidden) messages.push({
    id: "operation-error", kind: "error", title: commandErrorText(error, t),
    onDismiss: operation.dismiss,
  });
  if (preparationId && !prepared.hidden) {
    messages.push({ id: preparationId, kind: preparation.issues.length ? "warning" : "info",
      title: preparation.issues.length ? t("preparation.partial")
        : t("preparation.repaired", { count: preparation.repairedFiles.length }),
      description: <PreparationDetails snapshot={preparation} />, onDismiss: prepared.dismiss });
  }
  const toastMessages = queue.toasts.map((toast): InformationMessage => ({
    id: `toast-${toast.id}`, toastId: toast.id, kind: toast.kind,
    title: toast.title || toast.description, description: toast.title ? toast.description : undefined,
    onDismiss: () => queue.dismiss(toast.id),
  }));
  messages.push(...toastMessages.filter((message) => message.kind === "error"));
  if (busy && !processing.hidden) messages.push({
    id: "busy", kind: "info", title: t("shell.busy"), onDismiss: processing.dismiss,
  });
  messages.push(...toastMessages.filter((message) => message.kind !== "error"));
  return { ...queue, messages };
}

function InformationPanel({ id, messages, onClose }: {
  id: string;
  messages: readonly InformationMessage[];
  onClose: () => void;
}) {
  const { t } = useI18n();
  const closeRef = useRef<HTMLButtonElement>(null);
  const focusClose = () => closeRef.current?.focus({ preventScroll: true });
  return <Dialog id={id} className="asb-information-dialog" aria-label={t("shell.information.aria")}>
    <div className="asb-information-heading">
      <h2 className="asb-section-title">{t("shell.information.aria")}</h2>
      <div className="asb-information-heading-actions">
        <Button variant="secondary" size="compact" disabled={!messages.length}
          onClick={() => {
            focusClose();
            messages.forEach((message) => message.onDismiss());
          }}>
          {t("shell.information.clear")}
        </Button>
        <Button ref={closeRef} variant="icon" aria-label={t("shell.information.close")} onClick={onClose}>
          <CloseIcon />
        </Button>
      </div>
    </div>
    <ScrollArea className="asb-information-list" aria-label={t("shell.information.aria")}>
      {messages.map((message) =>
        <InformationRow key={message.id} message={message} onBeforeDismiss={focusClose} />)}
    </ScrollArea>
  </Dialog>;
}

function InformationContent({ primary, messages, toasts, pause, resume }: ReturnType<typeof useInformationMessages> & {
  primary: InformationMessage;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();
  const visibleToastId = primary.toastId;

  useEffect(() => {
    const syncTimers = () => {
      for (const toast of toasts) {
        // A queued message must not expire while another message owns the summary.
        if (document.hidden || open || hovered || focused || toast.id !== visibleToastId) pause(toast.id);
        else resume(toast.id);
      }
    };
    syncTimers();
    document.addEventListener("visibilitychange", syncTimers);
    return () => document.removeEventListener("visibilitychange", syncTimers);
  }, [toasts, visibleToastId, open, hovered, focused, pause, resume]);

  return (
    <>
      <Button variant="unstyled" ref={triggerRef} className="asb-information-trigger"
        aria-label={t("shell.information.show", { count: messages.length })}
        aria-describedby={`${panelId}-summary`}
        aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? panelId : undefined}
        onClick={() => setOpen(!open)}
        onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)}
        onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}>
        <ToastStatusIcon kind={primary.kind} />
        <div id={`${panelId}-summary`} className="asb-information-summary" role={primary.kind === "error" ? "alert" : "status"}
          aria-atomic="true" key={primary.id}>
          <ToastText content={primary.title} />
        </div>
        <span className="asb-information-count">{messages.length}</span>
      </Button>
      <Popover triggerRef={triggerRef} isOpen={open} onOpenChange={setOpen} placement="bottom start"
        className="asb-information-popover" offset={8}>
        <InformationPanel id={panelId} messages={messages} onClose={() => setOpen(false)} />
      </Popover>
    </>
  );
}

/** The titlebar owns the only global feedback surface; source state stays with its owners. */
export function GlobalInformation(props: Props) {
  const information = useInformationMessages(props);
  const primary = information.messages[0];
  return (
    <div className="asb-global-information" data-tauri-drag-region>
      {primary && <InformationContent {...information} primary={primary} />}
    </div>
  );
}
