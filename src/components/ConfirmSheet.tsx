import { useEffect, useRef, type ReactNode } from "react";

interface Props {
  title: string;
  details: ReactNode[];
  confirmLabel: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Confirmation sheet for irreversible actions. Escape cancels; focus starts
 * on the cancel button so a stray Enter cannot confirm by accident.
 */
export function ConfirmSheet({
  title,
  details,
  confirmLabel,
  destructive = false,
  onConfirm,
  onCancel,
}: Props) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const sheetRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onCancel();
        return;
      }
      if (event.key !== "Tab") return;

      const controls = sheetRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
      if (!controls || controls.length === 0) return;
      const first = controls[0];
      const last = controls[controls.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="asb-sheet-backdrop" onClick={(e) => e.target === e.currentTarget && onCancel()}>
      <div
        ref={sheetRef}
        className="asb-sheet"
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <h2 className="asb-panel-title">{title}</h2>
        <ul className="asb-sheet-details">
          {details.map((detail, index) => (
            <li key={index}>{detail}</li>
          ))}
        </ul>
        <div className="asb-sheet-actions">
          <button ref={cancelRef} type="button" className="asb-btn-secondary" onClick={onCancel}>
            取消
          </button>
          <button
            type="button"
            className={destructive ? "asb-btn-danger" : "asb-btn-primary"}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
