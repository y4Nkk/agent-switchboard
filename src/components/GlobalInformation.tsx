import { useEffect, useId, useRef, useState } from "react";
import { Dialog, Popover } from "react-aria-components";
import type { CommandError } from "../api/client";
import { canRepairApplicationData } from "../api/data-repair";
import { useI18n } from "../i18n";
import { commandErrorText } from "../i18n/errors";
import { Button } from "./Button";
import { CloseIcon, TrashIcon } from "./icons";
import { Tooltip } from "./Tooltip";
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
        {message.action && <div className="asb-information-actions">{message.action}</div>}
        {message.onAcknowledge && <div className="asb-information-actions">
          <Button variant="secondary" size="compact" aria-pressed={message.acknowledged ?? false}
            onClick={message.onAcknowledge}>
            {t(message.acknowledged ? "notice.acknowledged" : "notice.acknowledge")}
          </Button>
        </div>}
      </div>
      {message.onDismiss && (
        <Tooltip label={t("shell.information.delete")} className="asb-information-tooltip">
          <Button variant="icon" aria-label={t("shell.information.delete")} onClick={() => {
            onBeforeDismiss();
            message.onDismiss?.();
          }}>
            <TrashIcon />
          </Button>
        </Tooltip>
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
    onDismiss: () => queue.dismiss(toast.id),
  }));
  messages.push(...toastMessages.filter((message) => message.kind === "error"));
  messages.push(...sources.filter((message) => !message.acknowledged && message.kind === "error"));
  messages.push(...sources.filter((message) => !message.acknowledged && message.kind === "warning"));
  if (busy) messages.push({ id: "busy", kind: "info", title: t("shell.busy") });
  messages.push(...toastMessages.filter((message) => message.kind !== "error"));
  messages.push(...sources.filter((message) => message.acknowledged || message.kind !== "error" && message.kind !== "warning"));
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
  const dismissible = messages.filter((message) => message.onDismiss);
  return <Dialog id={id} className="asb-information-dialog" aria-label={t("shell.information.aria")}>
    <div className="asb-information-heading">
      <h2 className="asb-section-title">{t("shell.information.aria")}</h2>
      <div className="asb-information-heading-actions">
        <Button variant="secondary" size="compact" disabled={!dismissible.length}
          onClick={() => {
            focusClose();
            dismissible.forEach((message) => message.onDismiss?.());
          }}>
          {t("shell.information.clear")}
        </Button>
        <Button ref={closeRef} variant="icon" aria-label={t("shell.information.close")} onClick={onClose}>
          <CloseIcon />
        </Button>
      </div>
    </div>
    <ScrollArea className="asb-information-list" aria-label={t("shell.information.aria")}>
      {messages.length ? messages.map((message) =>
        <InformationRow key={message.id} message={message} onBeforeDismiss={focusClose} />)
        : <p className="asb-information-empty" role="status">{t("shell.information.empty")}</p>}
    </ScrollArea>
  </Dialog>;
}

/** The titlebar owns the only global feedback surface; source state stays with its owners. */
export function GlobalInformation(props: Props) {
  const { t } = useI18n();
  const { messages, toasts, pause, resume } = useInformationMessages(props);
  const [open, setOpen] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();
  const primary: InformationMessage = messages.find((message) => !message.acknowledged)
    ?? { id: messages.length ? "acknowledged" : "empty", kind: "info",
      title: messages.length ? t("notice.review", { count: messages.length }) : t("shell.information.empty") };
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
    <div className="asb-global-information" data-tauri-drag-region
      onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)} onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}>
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
        <InformationPanel id={panelId} messages={messages} onClose={() => setOpen(false)} />
      </Popover>
    </div>
  );
}
