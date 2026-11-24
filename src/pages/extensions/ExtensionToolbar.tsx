import type { ReactNode } from "react";
import { EXTENSION_SECTIONS, type ExtensionSection } from "../../app/navigation";
import { clientDeployState } from "../../app/extensions/deployment-state";
import { useI18n, type TFunction } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Button } from "../../components/Button";
import { ActionMenu, type ActionMenuItem } from "../../components/ActionMenu";
import { SearchInput } from "../../components/SearchInput";
import { Tabs } from "../../components/Tabs";
import { WorkspaceHeader } from "../../components/WorkspaceHeader";
import { McpKindIcon, SkillKindIcon, UpdateIcon } from "../../components/icons";
import { MANAGEMENT_CLIENTS } from "../../components/extensions/client-presentation";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

const KIND_ICONS = { skill: SkillKindIcon, mcp: McpKindIcon } as const;

function extensionTabs(workspace: ExtensionWorkspace, t: TFunction) {
  return EXTENSION_SECTIONS.map((tab) => {
    const KindIcon = KIND_ICONS[tab.value];
    return {
      ...tab,
      label: (
        <>
          <KindIcon />
          <span>{t(tab.labelKey)}</span>
          <span className="asb-ext-tab-count asb-num">
            {workspace.items.filter((item) => item.kind === tab.value).length}
          </span>
        </>
      ),
      controls: `ext-workspace-${tab.value}-panel`,
    };
  });
}

export function ExtensionSearch({ kind, search, onSearch, actions }: {
  kind: ExtensionSection;
  search: string;
  onSearch: (value: string) => void;
  actions?: ReactNode;
}) {
  const { t } = useI18n();
  return (
    <div className="asb-ext-toolbar">
      <div className="asb-ext-search">
        <SearchInput
          placeholder={kind === "skill" ? t("extensions.toolbar.searchSkills") : t("extensions.toolbar.searchMcp")}
          ariaLabel={t("extensions.toolbar.searchAria")}
          value={search}
          onChange={onSearch}
        />
      </div>
      {actions && <div className="asb-ext-toolbar-actions">{actions}</div>}
    </div>
  );
}

function ExtensionMoreMenu({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const { nav, writeBlocked, kindItems, busy } = workspace;
  const skills = nav.kind === "skill";
  const bulkActions = MANAGEMENT_CLIENTS.map((client): ActionMenuItem => {
    const state = clientDeployState(kindItems, client);
    const action = t(state.all ? "extensions.toolbar.disableAll" : "extensions.toolbar.enableAll", { client: clientName(client) });
    return {
      id: `bulk-${client}`,
      label: t(nav.search.trim() ? "extensions.toolbar.bulkFiltered" : "extensions.toolbar.bulkAction", { action, count: state.enabled }),
      disabled: writeBlocked || state.applicable === 0,
      onAction: () => void workspace.toggleAll(client),
    };
  });
  const items: ActionMenuItem[] = [
    { id: "new", label: t(skills ? "extensions.toolbar.newSkill" : "extensions.toolbar.addMcp"), disabled: writeBlocked,
      onAction: () => nav.setDialog(skills ? { type: "newSkill" } : { type: "newMcp" }) },
    { id: "discovery", label: t("extensions.discovery.title"), disabled: busy,
      onAction: () => nav.setDiscoveryOpen(true) },
    { id: "portable", label: t("extensions.toolbar.importPortable"), disabled: writeBlocked,
      onAction: () => nav.setDialog({ type: "portableImport" }) },
    { id: "project", label: t("extensions.toolbar.registerProject"), disabled: writeBlocked,
      onAction: () => nav.setDialog({ type: "project" }) },
    ...bulkActions,
    { id: "history", label: t("extensions.toolbar.history"), disabled: busy,
      onAction: () => nav.setDialog({ type: "history" }) },
  ];
  return <ActionMenu label={t("extensions.toolbar.more")} ariaLabel={t("extensions.toolbar.moreAria")} items={items} />;
}

function McpUpdateResults({ workspace: w }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const items = w.kindItems.flatMap((item): ActionMenuItem[] => {
    const report = w.registryUpdates.reportMap.get(item.id);
    if (report?.status !== "available" && report?.status !== "manual") return [];
    const status = report.status === "manual" ? t("extensions.registry.manualUpdate", { version: report.version })
      : t(report.action === "associateCurrent" ? "extensions.registry.associateVersion" : "extensions.list.updatable");
    return [{
      id: item.id,
      label: `${item.name} · ${status}`,
      disabled: w.writeBlocked,
      onAction: () => {
        if (report.status === "manual") w.nav.openManagement(item.id, item.kind);
        else w.nav.setDialog({ type: "mcpRegistryUpdate", report });
      },
    }];
  });
  if (items.length === 0) return null;
  const label = t("extensions.registry.updateResults", { count: items.length });
  return <ActionMenu label={label} ariaLabel={label} items={items} disabled={w.busy} />;
}

function ExtensionLibraryActions({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const { nav, writeBlocked, updates, registryUpdates, kindItems } = workspace;
  const skills = nav.kind === "skill";
  const onlineIds = kindItems.filter((item) => item.kind === "skill" ? item.source?.resolvedCommit : item.mcpMetadata?.registry)
    .map((item) => item.id);
  return (
    <>
      {onlineIds.length > 0 && (
        <Button variant="secondary" disabled={writeBlocked}
          onClick={() => void (skills ? updates.check(onlineIds) : registryUpdates.check(onlineIds))}>
          <UpdateIcon />
          {t("extensions.management.checkUpdates")}
        </Button>
      )}
      {skills && updates.updatable.length > 0 && (
        <Button
          variant="secondary"
          disabled={writeBlocked}
          onClick={() => void updates.update(updates.updatable)}
        >
          <UpdateIcon />
          {t("extensions.toolbar.updateAll", { count: updates.updatable.length })}
        </Button>
      )}
      {!skills && <McpUpdateResults workspace={workspace} />}
      <ExtensionMoreMenu workspace={workspace} />
    </>
  );
}

export function ExtensionToolbar({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const { nav } = workspace;
  const controls = nav.view === "library" ? (
    <ExtensionSearch
      kind={nav.kind}
      search={nav.search}
      onSearch={nav.setSearch}
      actions={<ExtensionLibraryActions workspace={workspace} />}
    />
  ) : undefined;
  return (
    <WorkspaceHeader
      title={t("nav.page.extensions")}
      primary={
        <Tabs value={nav.section} onChange={nav.changeSection}
          tabs={extensionTabs(workspace, t)} scope="ext-workspace" label={t("extensions.toolbar.tabsAria")} />
      }
      primaryActions={<div className="asb-ext-views" role="group" aria-label={t("extensions.toolbar.viewAria")}>
        <Button variant="secondary" aria-pressed={nav.view === "library"} onClick={() => nav.changeView("library")}>
          {t("extensions.toolbar.libraryView")}
        </Button>
        <Button ref={nav.marketButton} variant="secondary" aria-pressed={nav.view === "market"} onClick={nav.openMarket}>
          {t("extensions.toolbar.marketView")}
        </Button>
      </div>}
      secondary={controls}
    />
  );
}
