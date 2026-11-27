import type { ReactNode } from "react";
import { RefreshCw } from "lucide-react";
import { Button } from "../Button";
import { SettingsGroup } from "../SettingsLayout";
import { Tooltip } from "../Tooltip";

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
  return (
    <SettingsGroup className="asb-client-management-module" title={title} busy={busy} surface="plain" actions={
        <Tooltip label={refreshLabel}>
          <Button variant="icon" aria-label={refreshLabel}
            disabled={refreshDisabled ?? busy} onClick={onRefresh}><RefreshCw aria-hidden="true" /></Button>
        </Tooltip>
    }>
      {children}
    </SettingsGroup>
  );
}
