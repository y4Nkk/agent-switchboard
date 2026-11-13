import type { Icon as NavIcon } from "@phosphor-icons/react";
import { useLayoutEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { PAGES, pageLabelKey, type Page } from "./navigation";
import { Button } from "../components/Button";
import { Tooltip } from "../components/Tooltip";
import { UpdateButton } from "../components/UpdateButton";
import type { ApplicationUpdateControl } from "./update-state";
import { SideNavIndicator } from "./SideNavIndicator";
import {
  ClientConfigurationIcon,
  ExtensionsIcon,
  GatewayNavigationIcon,
  ProviderSwitchIcon,
  SessionsIcon,
  SettingsIcon,
  UsageNavigationIcon,
  CollapseNavigationIcon,
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
  /** App owns the update task; navigation only forwards its snapshot. */
  update: ApplicationUpdateControl;
  onOpenUpdate: () => void;
}

/** Navigation stays mounted across width changes; its group scrolls independently.
 * Each list owns one indicator instance, so the pill only ever moves within
 * the list that holds the current page. */
export function SideNav({ page, onPageChange, collapsed, pending, onToggle, update, onOpenUpdate }: SideNavProps) {
  const { t } = useI18n();
  // A descendant's layout effect can run before the list ref attaches.
  // Publishing the node re-runs indicator setup once the container exists.
  const [group, setGroup] = useState<HTMLUListElement | null>(null);
  const [footer, setFooter] = useState<HTMLUListElement | null>(null);
  return (
    <aside className="asb-sidebar">
      <div className="asb-sidebar-top" data-tauri-drag-region>
        <Button variant="unstyled" className="asb-sidebar-toggle"
          aria-label={t(collapsed === null ? "shell.nav.retry" : collapsed ? "shell.nav.expand" : "shell.nav.collapse")}
          aria-expanded={collapsed === null ? undefined : !collapsed}
          aria-controls="asb-primary-navigation" disabled={pending} aria-busy={pending} onClick={onToggle}>
          {collapsed === null ? <ReloadIcon /> : (
            <span className="asb-sidebar-toggle-glyph" aria-hidden="true"><CollapseNavigationIcon /></span>
          )}
        </Button>
      </div>
      <nav id="asb-primary-navigation" className="asb-sidenav" aria-label={t("shell.nav.aria")}>
        <ul className="asb-sidenav-group" ref={setGroup}>
          {WORKSPACE_PAGES.map((item) => (
            <li key={item}>
              <SideNavItem page={item} active={page === item} collapsed={collapsed === true}
                onSelect={() => onPageChange(item)} />
            </li>
          ))}
          <SideNavIndicator list={group} current={page === "settings" ? null : page} />
        </ul>
        <ul className="asb-sidenav-footer" ref={setFooter}>
          <li>
            <UpdateButton update={update} collapsed={collapsed === true} onOpen={onOpenUpdate} />
          </li>
          <li>
            <SideNavItem page="settings" active={page === "settings"} collapsed={collapsed === true}
              onSelect={() => onPageChange("settings")} />
          </li>
          <SideNavIndicator list={footer} current={page === "settings" ? page : null} />
        </ul>
      </nav>
    </aside>
  );
}

function SideNavItem({ page, active, collapsed, onSelect }: {
  page: Page;
  active: boolean;
  collapsed: boolean;
  onSelect: () => void;
}) {
  const { t } = useI18n();
  const label = t(pageLabelKey(page));
  const Icon = PAGE_ICONS[page];
  return (
    <Tooltip label={label} side="right" enabled={collapsed}>
      <Button
        variant="unstyled"
        className="asb-sidenav-item"
        aria-label={label}
        aria-current={active ? "page" : undefined}
        onClick={onSelect}
      >
        <SideNavGlyph icon={Icon} active={active} />
        <span className="asb-sidenav-label" aria-hidden="true">{label}</span>
      </Button>
    </Tooltip>
  );
}

function SideNavGlyph({ icon: Icon, active }: { icon: NavIcon; active: boolean }) {
  const ref = useRef<HTMLSpanElement>(null);
  const wasActive = useRef(active);
  useLayoutEffect(() => {
    const entering = active && !wasActive.current;
    wasActive.current = active;
    const node = ref.current;
    node?.toggleAttribute("data-entering", entering);
  }, [active]);
  return (
    <span ref={ref} className="asb-sidenav-glyph" aria-hidden="true">
      <Icon size={24} weight="regular" focusable="false" />
      <Icon size={24} weight="fill" focusable="false" />
    </span>
  );
}
