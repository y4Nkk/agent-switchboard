import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState,
  type ReactNode, type RefObject } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { CloseIcon } from "./icons";
import { OverlayMotionContext } from "./overlay-motion";
import { ScrollArea } from "./ScrollArea";

interface Props {
  title: string;
  busy: boolean;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  narrow?: boolean;
  tall?: boolean;
}

const AppDialogSuspensionContext = createContext(false);
const AppDialogFieldContext = createContext<{
  surface: RefObject<HTMLElement | null>;
  suspended: boolean;
  blockDismissal: () => () => void;
} | null>(null);

export function useDialogFieldPopup(disabled: boolean, blocksDialogDismissal = false) {
  const dialog = useContext(AppDialogFieldContext);
  const [popup, setPopup] = useState<{ container?: HTMLElement } | null>(null);
  const blocked = disabled || dialog?.suspended === true;
  const open = !blocked && popup !== null;
  useEffect(() => {
    if (blocked) setPopup(null);
  }, [blocked]);
  useLayoutEffect(() => {
    if (open && blocksDialogDismissal) return dialog?.blockDismissal();
  }, [open, blocksDialogDismissal, dialog]);
  const onOpenChange = (nextOpen: boolean) => {
    setPopup(nextOpen && !blocked ? { container: dialog?.surface.current ?? undefined } : null);
  };
  return { open, onOpenChange, container: popup?.container, disabled: blocked };
}

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
export function AppDialog({ title, busy, onClose, children, footer, wide = false, narrow = false, tall = false }: Props) {
  const suspended = useContext(AppDialogSuspensionContext);
  const animate = useContext(OverlayMotionContext);
  const { t } = useI18n();
  const surface = useRef<HTMLElement>(null);
  const [dismissalLocks, setDismissalLocks] = useState(0);
  const blockDismissal = useCallback(() => {
    setDismissalLocks((count) => count + 1);
    return () => setDismissalLocks((count) => count - 1);
  }, []);
  const fieldScope = useMemo(() => ({ surface, suspended, blockDismissal }), [suspended, blockDismissal]);
  const dismissable = !busy && !suspended && dismissalLocks === 0;
  const width = wide ? " is-wide" : narrow ? " is-narrow" : "";
  return (
    <ModalOverlay
      isOpen
      isDismissable={dismissable}
      isKeyboardDismissDisabled={!dismissable}
      onOpenChange={(open) => {
        if (!open && dismissable) onClose();
      }}
      aria-hidden={suspended || undefined}
      data-overlay-motion={animate || undefined}
      className={`asb-dialog-backdrop${suspended ? " is-suspended" : ""}`}
    >
      <Modal className={`asb-dialog${width}${tall ? " is-tall" : ""}`}>
        <Dialog ref={surface} aria-label={title} aria-busy={busy} className="asb-dialog-surface">
          <AppDialogFieldContext.Provider value={fieldScope}>
            <header className="asb-dialog-heading">
              <Heading slot="title" className="asb-dialog-title">
                {title}
              </Heading>
              <Button variant="icon" disabled={!dismissable} aria-label={t("window.closeDialog")} onClick={onClose}>
                <CloseIcon />
              </Button>
            </header>
            <ScrollArea className="asb-dialog-body" aria-label={title}>{children}</ScrollArea>
            {footer && <footer className="asb-dialog-footer">{footer}</footer>}
          </AppDialogFieldContext.Provider>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
