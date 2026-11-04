import { useEffect, useState } from "react";
import type { ExtensionsDeps } from "../app/extensions/extension-ops";
import { pickDirectory, pickFile } from "../api/client";
import { useI18n } from "../i18n";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { EXTENSION_SECTIONS } from "../app/navigation";
import { Button } from "../components/Button";
import { SkillSourceBrowser } from "../components/extensions/SkillSourceBrowser";
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
  const [visited, setVisited] = useState({ discovery: false, sources: false });
  useEffect(() => {
    if (nav.discoveryOpen || nav.sourceBrowser) setVisited((current) => ({
      discovery: current.discovery || nav.discoveryOpen, sources: current.sources || nav.sourceBrowser,
    }));
  }, [nav.discoveryOpen, nav.sourceBrowser]);
  const editing = nav.dialog?.type === "skillEditor" || nav.dialog?.type === "mcpEditor" || nav.dialog?.type === "newMcp";
  const discovery = nav.discoveryOpen;
  const sources = nav.sourceBrowser && nav.kind === "skill";
  const recovery = workspace.ext.workspace?.recoveryRequired ?? [];
  const notice = recovery.length > 0 ? <div className="asb-banner asb-banner-error" role="alert">
    <span>{t("extensions.recoveryBanner.text", { detail: recovery.join(t("extensions.join.semicolon")) })}</span>
    <Button variant="secondary" disabled={workspace.busy} onClick={() => void workspace.ext.recoverTransactions()}>
      {t("extensions.recoveryBanner.recover")}</Button></div> : null;
  return <>
    <div className="asb-editor-route" hidden={editing || !discovery}>
      {(visited.discovery || discovery) && <ExtensionDiscoveryView workspace={workspace} notice={notice} />}</div>
    <div className="asb-editor-route" hidden={editing || !sources || discovery}>
      {(visited.sources || sources) && <SkillSourceBrowser busy={workspace.writeBlocked} items={workspace.items}
        active={visible && sources && !discovery && !editing} retain={workspace.busy || nav.dialog !== null}
        onScanLocal={workspace.ext.scanLocal} onImport={workspace.importCandidate} onPickDirectory={pickDirectory}
        onPickZip={() => pickFile("ZIP archives", ["zip"])} onBack={() => nav.setSourceBrowser(false)} notice={notice} />}</div>
    <section className="asb-ext" aria-label={t("nav.page.extensions")} hidden={editing || discovery || sources}>
      <ExtensionToolbar workspace={workspace} />{notice}{EXTENSION_SECTIONS.map(({ value }) => <div key={value}
        id={`ext-workspace-${value}-panel`} role="tabpanel" aria-labelledby={`ext-workspace-${value}-tab`} hidden={nav.section !== value}>
        {nav.section === value && <ExtensionLibraryPanel workspace={workspace} />}</div>)}</section>
    <div className={editing ? "asb-editor-route" : undefined}><ExtensionWorkspaceDialogs workspace={workspace} /></div>
  </>;
}
