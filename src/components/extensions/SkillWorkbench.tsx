import { useRef, useState } from "react";
import type { ExtensionListItem, SkillDependency, SkillEditorView, SkillVersion } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { SettingsGroup, SettingsLayout } from "../SettingsLayout";
import { WorkspaceSubpage } from "../WorkspaceSubpage";
import { Tabs } from "../Tabs";
import { useUnsavedChanges } from "../useUnsavedChanges";
import { ExtensionLoading } from "./ExtensionLoading";
import { SkillDependencies, SkillFiles, SkillVersions } from "./SkillWorkbenchSections";
import { useSkillDependencies, useSkillFiles } from "./useSkillWorkbench";

export interface SkillWorkbenchHandlers {
  onLoadEditor: (id: string) => Promise<SkillEditorView | null>;
  onSaveFiles: (id: string, update: { expectedRevision: number; expectedDigest: string;
    files: Array<{ relativePath: string; text: string }> }) => Promise<unknown>;
  onLoadVersions: (id: string) => Promise<SkillVersion[] | null>;
  onRestoreVersion: (id: string, digest: string) => Promise<boolean>;
  onSaveDependencies: (id: string, update: { expectedRevision: number; dependencies: SkillDependency[] }) => Promise<unknown>;
  onFork: (id: string) => void;
  onClose: () => void;
}

export interface SkillWorkbenchProps extends SkillWorkbenchHandlers {
  item: Extract<ExtensionListItem, { kind: "skill" }>;
  mcpOptions: Array<{ id: string; name: string }>;
  busy: boolean;
}

type Section = "files" | "versions" | "dependencies";

/** Files and dependencies keep separate drafts; saving one never replaces the other. */
export function SkillWorkbench(props: SkillWorkbenchProps) {
  const { t } = useI18n();
  const [section, setSection] = useState<Section>("files");
  const [working, setWorking] = useState(false);
  const running = useRef(false);
  const files = useSkillFiles(props);
  const deps = useSkillDependencies(props.item.dependencies);
  const busy = props.busy || working;
  const leave = useUnsavedChanges(files.dirty || deps.dirty, busy, { guardPageRoot: true });
  const perform = async (action: () => Promise<unknown>) => {
    if (busy || running.current) return;
    running.current = true;
    setWorking(true);
    try { await action(); } finally { running.current = false; setWorking(false); }
  };
  const saveDependencies = async () => {
    if (!files.document || deps.stale) return;
    const draft = deps.draft.filter((row) => row.name.trim() !== "");
    const saved = await props.onSaveDependencies(props.item.id, {
      expectedRevision: props.item.revision, dependencies: draft,
    });
    if (saved) deps.reset(draft);
  };
  const restore = (digest: string) => leave.request(() => void perform(async () => {
    if (await props.onRestoreVersion(props.item.id, digest)) { await files.reload(); deps.reset(); }
  }));
  return <WorkspaceSubpage layout="fixed-actions" title={t("extensions.workbench.title", { name: props.item.name })}
    backLabel={t("extensions.backToLibrary")} busy={busy} onBack={() => leave.request(props.onClose)}
    primary={<Tabs value={section} onChange={setSection} scope="skill-workbench"
      label={t("extensions.workbench.aria", { name: props.item.name })}
      tabs={(["files", "versions", "dependencies"] as const).map((value) => ({ value,
        label: t(`extensions.workbench.${value}`), controls: `skill-workbench-${value}-panel` }))} />}
    footer={<><Button variant="secondary" disabled={busy} onClick={() => leave.request(props.onClose)}>{t("confirm.cancel")}</Button>
      {section === "files" && files.document?.view.editable && <Button variant="primary"
        disabled={busy || !files.dirty || files.document.stale} onClick={() => void perform(files.save)}>{t("extensions.workbench.saveVersion")}</Button>}
      {section === "dependencies" && <Button variant="primary" disabled={busy || !deps.dirty || deps.stale || !files.document}
        onClick={() => void perform(saveDependencies)}>{t("extensions.workbench.saveDeps")}</Button>}</>}>
    <SettingsLayout>
      {files.document?.stale && <div className="asb-banner" role="alert">{t("extensions.workbench.sourceChanged")}
        <Button variant="secondary" disabled={busy} onClick={() => leave.request(() => void perform(files.reload))}>{t("extensions.library.reload")}</Button></div>}
      <div role="tabpanel" id="skill-workbench-files-panel" aria-labelledby="skill-workbench-files-tab" hidden={section !== "files"}>
        {!files.document ? files.loading ? <ExtensionLoading /> : <div className="asb-banner" role="alert">
          {t("extensions.workbench.loadFailed")}<Button variant="secondary" disabled={busy}
            onClick={() => void perform(files.reload)}>{t("extensions.library.reload")}</Button></div> : !files.document.view.editable ? (
          <SettingsGroup title={t("extensions.workbench.files")} description={t("extensions.workbench.forkNote")} actions={
            <Button variant="primary" disabled={busy}
              onClick={() => leave.request(() => props.onFork(props.item.id))}>{t("extensions.workbench.fork")}</Button>
          }>{null}</SettingsGroup>
        ) : <SkillFiles files={files} busy={busy || files.document.stale} />}
      </div>
      <div role="tabpanel" id="skill-workbench-versions-panel" aria-labelledby="skill-workbench-versions-tab" hidden={section !== "versions"}>
        <SkillVersions id={props.item.id} revision={props.item.revision} busy={busy} onLoad={props.onLoadVersions} onRestore={restore} /></div>
      <div role="tabpanel" id="skill-workbench-dependencies-panel" aria-labelledby="skill-workbench-dependencies-tab" hidden={section !== "dependencies"}>
        <SkillDependencies deps={deps} options={props.mcpOptions} busy={busy} onReload={() => leave.request(() => deps.reset())} /></div>
    </SettingsLayout>
    {leave.confirmation}
  </WorkspaceSubpage>;
}
