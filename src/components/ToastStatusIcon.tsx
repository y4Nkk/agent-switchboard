import type { ToastKind } from "./use-toast";

interface ToastStatusIconProps {
  kind: ToastKind;
}

const ICON_TONE_CLASS: Record<ToastKind, string> = {
  info: "asb-toast-icon--info",
  success: "asb-toast-icon--success",
  warning: "asb-toast-icon--warning",
  error: "asb-toast-icon--error",
};

const ICON_PROPS = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  strokeWidth: 1.8,
};

function InfoSymbol() {
  return (
    <>
      <circle cx="12" cy="12" r="8.5" {...ICON_PROPS} />
      <circle cx="12" cy="8" r="1" fill="currentColor" stroke="none" />
      <path d="M12 11.2v5.1" {...ICON_PROPS} />
    </>
  );
}

function SuccessSymbol() {
  return (
    <>
      <circle cx="12" cy="12" r="8.5" {...ICON_PROPS} />
      <path d="m7.8 12.1 2.8 2.8 5.7-5.8" {...ICON_PROPS} />
    </>
  );
}

function WarningSymbol() {
  return (
    <>
      <path d="m12 3.7 8.7 16H3.3L12 3.7Z" {...ICON_PROPS} />
      <path d="M12 9v4.6" {...ICON_PROPS} />
      <circle cx="12" cy="16.7" r="0.9" fill="currentColor" stroke="none" />
    </>
  );
}

function ErrorSymbol() {
  return (
    <>
      <circle cx="12" cy="12" r="8.5" {...ICON_PROPS} />
      <path d="m8.8 8.8 6.4 6.4m0-6.4-6.4 6.4" {...ICON_PROPS} />
    </>
  );
}

function SymbolArt({ kind }: ToastStatusIconProps) {
  switch (kind) {
    case "info":
      return <InfoSymbol />;
    case "success":
      return <SuccessSymbol />;
    case "warning":
      return <WarningSymbol />;
    case "error":
      return <ErrorSymbol />;
  }
}

export function ToastStatusIcon({ kind }: ToastStatusIconProps) {
  return (
    <svg
      aria-hidden="true"
      className={`asb-toast-icon ${ICON_TONE_CLASS[kind]}`}
      focusable="false"
      viewBox="0 0 24 24"
      width={20}
      height={20}
    >
      <SymbolArt kind={kind} />
    </svg>
  );
}
