import { createContext, useContext, type ReactNode } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { CloseIcon } from "./icons";
import { ScrollArea } from "./ScrollArea";

interface Props {
  title: string;
  busy: boolean;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  narrow?: boolean;
}

const AppDialogSuspensionContext = createContext(false);

export function AppDialogSuspension({ suspended, children }: {
  suspended: boolean;
  children: ReactNode;
}) {
  const parentSuspended = useContext(AppDialogSuspensionContext);
  return (
    <AppDialogSuspensionContext.Provider value={parentSuspended || suspended}>
      {children}
    </AppDialogSuspensionContext.Provider>
  );
}

/**
 * The app's modal wrapper. Structure and geometry belong to the `.asb-dialog*`
 * skeleton in styles/base/overlays.css — this component only supplies React
 * Aria's focus isolation, the title slot and the close affordance, so every
 * dialog in the app lands on one anatomy and one of three widths.
 */
export function AppDialog({ title, busy, onClose, children, footer, wide = false, narrow = false }: Props) {
  const suspended = useContext(AppDialogSuspensionContext);
  const { t } = useI18n();
  return (
    <ModalOverlay
      isOpen
      isDismissable={!busy && !suspended}
      isKeyboardDismissDisabled={busy || suspended}
      onOpenChange={(open) => {
        if (!open && !busy && !suspended) onClose();
      }}
      aria-hidden={suspended || undefined}
      className={`asb-dialog-backdrop${suspended ? " is-suspended" : ""}`}
    >
      <Modal className={`asb-dialog${wide ? " is-wide" : narrow ? " is-narrow" : ""}`}>
        <Dialog aria-label={title} aria-busy={busy} className="asb-dialog-surface">
          <header className="asb-dialog-heading">
            <Heading slot="title" className="asb-dialog-title">
              {title}
            </Heading>
            <Button variant="icon" disabled={busy || suspended} aria-label={t("window.closeDialog")} onClick={onClose}>
              <CloseIcon />
            </Button>
          </header>
          <ScrollArea className="asb-dialog-body" aria-label={title}>{children}</ScrollArea>
          {footer && <footer className="asb-dialog-footer">{footer}</footer>}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
