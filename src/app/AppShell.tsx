import type { ReactNode } from "react";
import type { CommandError } from "../api/client";
import { useI18n } from "../i18n";
import type { Page } from "./navigation";
import { SideNav } from "./SideNav";
import { PinTopButton } from "../components/PinTopButton";
import { GlobalInformation } from "../components/GlobalInformation";
import { InformationSources } from "../components/InformationSources";
import { WindowControls } from "../components/WindowControls";
import { isBrowserDevelopment } from "../lib/runtime";
import appIcon from "../assets/app-icon.svg";

interface AppShellProps {
  page: Page;
  onPageChange: (page: Page) => void;
  sidebar: { collapsed: boolean | null; pending: boolean; onToggle: () => void };
  /** Persistent decision errors, displayed alongside transient global feedback. */
  error: CommandError | null;
  busy: boolean;
  /** Why application settings could not load; global information keeps the repair
   * reachable from every page while settings-backed actions silently wait. */
  settingsError: string | null;
  /** Replaces an unreadable settings file with validated defaults. */
  onRepairSettings: () => void;
  /** Repairs application-owned provider and outbound-proxy data. */
  onRepairData: () => void;
  /** Always-on-top toggle state; null until settings load. */
  pin: { active: boolean; onToggle: () => void } | null;
  /** Sidebar update entry above the settings item; App owns the check state. */
  update: { checking: boolean; latestVersion: string | null; onCheck: () => void; onOpen: () => void };
  children: ReactNode;
}

/** One native glass frame surrounds the bounded, rounded workspace. The rail
 * and titlebar reserve their own grid cells; page scrolling stays inside the
 * workspace and cannot pass beneath navigation or window actions. */
export function AppShell({
  page,
  onPageChange,
  sidebar,
  error,
  busy,
  settingsError,
  onRepairSettings,
  onRepairData,
  pin,
  update,
  children,
}: AppShellProps) {
  const { t } = useI18n();
  return (
    <InformationSources><div className={`asb-shell${sidebar.collapsed ? " is-collapsed" : ""}`}>
      <SideNav page={page} onPageChange={onPageChange} collapsed={sidebar.collapsed}
        onToggle={sidebar.onToggle} pending={sidebar.pending} update={update} />
      <header className="asb-titlebar" data-tauri-drag-region>
        <span className="asb-titlebar-brand" data-tauri-drag-region>
          <img className="asb-titlebar-icon" src={appIcon} alt="" data-tauri-drag-region />
          <span className="asb-titlebar-name" data-tauri-drag-region>Agent Switchboard</span>
        </span>
        <GlobalInformation error={error} settingsError={settingsError} busy={busy}
          onRepairData={onRepairData} onRepairSettings={onRepairSettings} />
        <div className="asb-titlebar-actions">
          {isBrowserDevelopment ? <span className="asb-web-development-badge">{t("shell.devBadge")}</span> : null}
          {pin ? (
            <PinTopButton active={pin.active} disabled={busy} onToggle={pin.onToggle} />
          ) : null}
          {!isBrowserDevelopment && <WindowControls />}
        </div>
      </header>
      <div className="asb-workspace">
        {children}
      </div>
    </div></InformationSources>
  );
}
