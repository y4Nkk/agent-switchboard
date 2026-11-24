import { useId, type ReactNode } from "react";
import { RefreshCw } from "lucide-react";
import { Button } from "../Button";

interface ClientManagementModuleProps {
  title: string;
  /** Accessible name of the icon-only refresh control. */
  refreshLabel: string;
  busy: boolean;
  refreshDisabled?: boolean;
  onRefresh: () => void;
  children: ReactNode;
}

/** Shared frame for the client-specific modules shown in each tab: a module
 * title row with its icon refresh action, then the module's own graphical
 * surfaces. */
export function ClientManagementModule({
  title,
  refreshLabel,
  busy,
  refreshDisabled,
  onRefresh,
  children,
}: ClientManagementModuleProps) {
  const titleId = useId();
  return (
    <section className="asb-client-management-module" aria-labelledby={titleId} aria-busy={busy || undefined}>
      <header className="asb-client-management-bar">
        <h3 id={titleId} className="asb-section-title">{title}</h3>
        <Button variant="icon" aria-label={refreshLabel} title={refreshLabel}
          disabled={refreshDisabled ?? busy} onClick={onRefresh}><RefreshCw /></Button>
      </header>
      {children}
    </section>
  );
}
