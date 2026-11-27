import { useEffect, useState } from "react";
import type { ExtensionsDeps } from "../app/extensions/extension-ops";
import { useI18n } from "../i18n";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { EXTENSION_SECTIONS, type ExtensionSection } from "../app/navigation";
import { useOnReturnToRoot } from "../app/PageRoot";
import { Button } from "../components/Button";
import { ExtensionMarketPanel } from "./extensions/ExtensionMarketPanel";
import { ExtensionLibraryPanel } from "./extensions/ExtensionLibraryPanel";
import { ExtensionToolbar } from "./extensions/ExtensionToolbar";
import { ExtensionDiscoveryView } from "./extensions/ExtensionDiscoveryView";
import { ExtensionWorkspaceDialogs } from "./extensions/ExtensionWorkspaceDialogs";
import type { ExtensionNavigation } from "./extensions/useExtensionView";
import { useExtensionWorkspace } from "./extensions/useExtensionWorkspace";

type ExtensionsPageProps = ExtensionsDeps & ExtensionNavigation & { active: boolean };

/** Keep the caller's source/search context behind full-page editors. */
export function ExtensionsPage(props: ExtensionsPageProps) {
  const { t } = useI18n();
  const visible = useWorkspaceVisible(props.active);
  const workspace = useExtensionWorkspace({ ...props, active: visible });
  const nav = workspace.nav;
  const editing = nav.dialog?.type === "skillEditor" || nav.dialog?.type === "mcpEditor" || nav.dialog?.type === "newMcp";
  const discovery = nav.discoveryOpen;
  useOnReturnToRoot(visible && (editing || discovery), () => {
    nav.setDialog(null);
    nav.setDiscoveryOpen(false);
  }, workspace.busy);
  const [visitedDiscovery, setVisitedDiscovery] = useState(false);
  const [visitedMarkets, setVisitedMarkets] = useState<Partial<Record<ExtensionSection, true>>>({});
  useEffect(() => {
    if (nav.discoveryOpen) setVisitedDiscovery(true);
  }, [nav.discoveryOpen]);
  useEffect(() => {
    if (nav.view === "market") setVisitedMarkets((current) =>
      current[nav.section] ? current : { ...current, [nav.section]: true });
  }, [nav.section, nav.view]);
  const market = nav.view === "market";
  const recovery = workspace.ext.workspace?.recoveryRequired ?? [];
  const notice = recovery.length > 0 ? <div className="asb-banner asb-banner-error" role="alert">
    <span>{t("extensions.recoveryBanner.text", { detail: recovery.join(t("extensions.join.semicolon")) })}</span>
    <Button variant="secondary" disabled={workspace.busy} onClick={() => void workspace.ext.recoverTransactions()}>
      {t("extensions.recoveryBanner.recover")}</Button></div> : null;
  return <>
    <div className="asb-subpage-route" hidden={editing || !discovery}>
      {(visitedDiscovery || discovery) && <ExtensionDiscoveryView workspace={workspace} notice={notice} />}</div>
    <section className="asb-ext" aria-label={t("nav.page.extensions")} hidden={editing || discovery}>
      <ExtensionToolbar workspace={workspace} />
      {notice}
      {EXTENSION_SECTIONS.map(({ value }) => {
        const selected = nav.section === value;
        const showMarket = selected && market;
        return <div key={value} id={`ext-workspace-${value}-panel`} role="tabpanel"
          aria-labelledby={`ext-workspace-${value}-tab`} hidden={!selected}>
          {selected && <div className="asb-ext-pane" hidden={market}>
            <ExtensionLibraryPanel workspace={workspace} />
          </div>}
          {(showMarket || visitedMarkets[value]) && <div className="asb-ext-pane" hidden={!showMarket}>
            <ExtensionMarketPanel workspace={workspace} kind={value} active={visible && showMarket && !editing && !discovery} />
          </div>}
        </div>;
      })}
    </section>
    <div className={editing ? "asb-subpage-route" : undefined}><ExtensionWorkspaceDialogs workspace={workspace} /></div>
  </>;
}
