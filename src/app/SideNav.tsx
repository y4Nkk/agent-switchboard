import type { Icon as NavIcon } from "@phosphor-icons/react";
import { useI18n } from "../i18n";
import { PAGES, pageLabelKey, type Page } from "./navigation";
import { Button } from "../components/Button";
import { Tooltip } from "../components/Tooltip";
import { UpdateButton } from "../components/UpdateButton";
import {
  ClientConfigurationIcon,
  ExtensionsIcon,
  GatewayNavigationIcon,
  ProviderSwitchIcon,
  SessionsIcon,
  SettingsIcon,
  UsageNavigationIcon,
  CollapseNavigationIcon,
  ExpandNavigationIcon,
  ReloadIcon,
} from "../components/icons";

/** Icon per top-level page; the mapping sits beside the rail, its single
 * consumer, while the page list itself stays owned by navigation.ts. */
const PAGE_ICONS: Record<Page, NavIcon> = {
  providers: ProviderSwitchIcon,
  gateway: GatewayNavigationIcon,
  clientConfiguration: ClientConfigurationIcon,
  extensions: ExtensionsIcon,
  sessions: SessionsIcon,
  usage: UsageNavigationIcon,
  settings: SettingsIcon,
};

/** Workspaces in the scrollable group; settings is the pinned footer entry. */
const WORKSPACE_PAGES = PAGES.filter((page) => page !== "settings");

interface SideNavProps {
  page: Page;
  onPageChange: (page: Page) => void;
  collapsed: boolean | null;
  pending: boolean;
  onToggle: () => void;
  /** Sidebar update entry above the settings item; App owns the check state. */
  update: { checking: boolean; latestVersion: string | null; onCheck: () => void; onOpen: () => void };
}

/** Navigation stays mounted across width changes; its group scrolls independently. */
export function SideNav({ page, onPageChange, collapsed, pending, onToggle, update }: SideNavProps) {
  const { t } = useI18n();
  return (
    <aside className="asb-sidebar">
      <div className="asb-sidebar-top" data-tauri-drag-region>
        <Button variant="icon" aria-label={t(collapsed === null ? "shell.nav.retry" : collapsed ? "shell.nav.expand" : "shell.nav.collapse")}
          aria-expanded={collapsed === null ? undefined : !collapsed}
          aria-controls="asb-primary-navigation" disabled={pending} aria-busy={pending} onClick={onToggle}>
          {collapsed === null ? <ReloadIcon /> : collapsed ? <ExpandNavigationIcon /> : <CollapseNavigationIcon />}
        </Button>
      </div>
      <nav id="asb-primary-navigation" className="asb-sidenav" aria-label={t("shell.nav.aria")}>
        <ul className="asb-sidenav-group">
          {WORKSPACE_PAGES.map((item) => (
            <li key={item}>
              <SideNavItem page={item} active={page === item}
                onSelect={() => onPageChange(item)} />
            </li>
          ))}
        </ul>
        <ul className="asb-sidenav-footer">
          <li>
            <UpdateButton checking={update.checking} latestVersion={update.latestVersion}
              onCheck={update.onCheck} onOpen={update.onOpen} />
          </li>
          <li>
            <SideNavItem page="settings" active={page === "settings"}
              onSelect={() => onPageChange("settings")} />
          </li>
        </ul>
      </nav>
    </aside>
  );
}

function SideNavItem({ page, active, onSelect }: {
  page: Page;
  active: boolean;
  onSelect: () => void;
}) {
  const { t } = useI18n();
  const label = t(pageLabelKey(page));
  const Icon = PAGE_ICONS[page];
  return (
    <Tooltip label={label} side="right">
      <Button
        variant="unstyled"
        className="asb-sidenav-item"
        aria-label={label}
        aria-current={active ? "page" : undefined}
        onClick={onSelect}
      >
        <Icon size={24} weight={active ? "fill" : "regular"} aria-hidden="true" focusable="false" />
        <span className="asb-sidenav-label" aria-hidden="true">{label}</span>
      </Button>
    </Tooltip>
  );
}
