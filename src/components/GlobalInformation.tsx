import { useEffect, useId, useRef, useState } from "react";
import { Dialog, Popover } from "react-aria-components";
import type { CommandError } from "../api/client";
import { canRepairApplicationData } from "../api/data-repair";
import { useI18n } from "../i18n";
import { commandErrorText } from "../i18n/errors";
import { Button } from "./Button";
import { CloseIcon } from "./icons";
import { ToastStatusIcon } from "./ToastStatusIcon";
import { ToastText } from "./ToastContent";
import { ScrollArea } from "./ScrollArea";
import { useToast } from "./use-toast";
import { useInformationSources, type InformationSourceMessage } from "./InformationSources";

interface Props {
  error: CommandError | null;
  settingsError: string | null;
  busy: boolean;
  onRepairData: () => void;
  onRepairSettings: () => void;
}

interface InformationMessage extends InformationSourceMessage {
  toastId?: string;
}

function InformationRow({ message, onDismiss }: {
  message: InformationMessage;
  onDismiss: (id: string) => void;
}) {
  const { t } = useI18n();
  const { toastId } = message;
  return (
    <div className="asb-information-row">
      <ToastStatusIcon kind={message.kind} />
      <div className="asb-information-copy">
        <div className="asb-information-title"><ToastText content={message.title} /></div>
        {message.description && (
          <div className="asb-information-description"><ToastText content={message.description} /></div>
        )}
        {message.action && <div className="asb-information-actions">{message.action}</div>}
        {message.onAcknowledge && <div className="asb-information-actions">
          <Button variant="secondary" size="compact" aria-pressed={message.acknowledged ?? false}
            onClick={message.onAcknowledge}>
            {t(message.acknowledged ? "notice.acknowledged" : "notice.acknowledge")}
          </Button>
        </div>}
      </div>
      {toastId && (
        <Button variant="icon" aria-label={t("toast.dismiss")} onClick={() => onDismiss(toastId)}>
          <CloseIcon />
        </Button>
      )}
    </div>
  );
}

function useInformationMessages({ error, settingsError, busy, onRepairData, onRepairSettings }: Props) {
  const { t } = useI18n();
  const queue = useToast();
  const sources = useInformationSources();
  const messages: InformationMessage[] = [];

  if (settingsError) messages.push({
    id: "settings-error", kind: "error", title: t("shell.banner.settingsUnavailable", { detail: settingsError }),
    action: <Button variant="secondary" disabled={busy} onClick={onRepairSettings}>{t("shell.banner.repair")}</Button>,
  });
  if (error) messages.push({
    id: "operation-error", kind: "error", title: commandErrorText(error, t),
    action: canRepairApplicationData(error)
      ? <Button variant="secondary" disabled={busy} onClick={onRepairData}>{t("shell.banner.repairData")}</Button>
      : undefined,
  });
  const toastMessages = queue.toasts.map((toast): InformationMessage => ({
    id: `toast-${toast.id}`, toastId: toast.id, kind: toast.kind,
    title: toast.title || toast.description, description: toast.title ? toast.description : undefined,
  }));
  messages.push(...toastMessages.filter((message) => message.kind === "error"));
  messages.push(...sources.filter((message) => !message.acknowledged && message.kind === "error"));
  messages.push(...sources.filter((message) => !message.acknowledged && message.kind === "warning"));
  if (busy) messages.push({ id: "busy", kind: "info", title: t("shell.busy") });
  messages.push(...toastMessages.filter((message) => message.kind !== "error"));
  messages.push(...sources.filter((message) => message.acknowledged || message.kind !== "error" && message.kind !== "warning"));
  return { ...queue, messages };
}

/** The titlebar owns the only global feedback surface; source state stays with its owners. */
export function GlobalInformation(props: Props) {
  const { t } = useI18n();
  const { messages, toasts, dismiss, pause, resume } = useInformationMessages(props);
  const [open, setOpen] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();
  const primary: InformationMessage | undefined = messages.find((message) => !message.acknowledged)
    ?? (messages.length ? { id: "acknowledged", kind: "info", title: t("notice.review", { count: messages.length }) } : undefined);
  const visibleToastId = primary?.toastId;

  useEffect(() => {
    if (messages.length === 0) setOpen(false);
  }, [messages.length]);

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
    <div className="asb-global-information" data-tauri-drag-region
      onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)} onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}>
      {primary && (
        <>
          <Button variant="unstyled" ref={triggerRef} className="asb-information-trigger"
            aria-label={t("shell.information.show", { count: messages.length })}
            aria-describedby={`${panelId}-summary`}
            aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? panelId : undefined}
            onClick={() => setOpen(!open)}>
            <ToastStatusIcon kind={primary.kind} />
            <div id={`${panelId}-summary`} className="asb-information-summary" role={primary.kind === "error" ? "alert" : "status"}
              aria-atomic="true" key={primary.id}>
              <ToastText content={primary.title} />
            </div>
            <span className="asb-information-count">{messages.length}</span>
          </Button>
          <Popover triggerRef={triggerRef} isOpen={open} onOpenChange={setOpen} placement="bottom start"
            className="asb-information-popover" offset={8}>
            <Dialog id={panelId} className="asb-information-dialog" aria-label={t("shell.information.aria")}>
              <div className="asb-information-heading">
                <h2 className="asb-section-title">{t("shell.information.aria")}</h2>
                <Button variant="icon" aria-label={t("shell.information.close")} onClick={() => setOpen(false)}>
                  <CloseIcon />
                </Button>
              </div>
              <ScrollArea className="asb-information-list" aria-label={t("shell.information.aria")}>
                {messages.map((message) => <InformationRow key={message.id} message={message} onDismiss={dismiss} />)}
              </ScrollArea>
            </Dialog>
          </Popover>
        </>
      )}
    </div>
  );
}
