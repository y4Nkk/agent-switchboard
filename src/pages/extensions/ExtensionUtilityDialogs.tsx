import { useId, useState } from "react";
import { pickDirectory, pickFile } from "../../api/client";
import { AppDialog } from "../../components/AppDialog";
import { Button } from "../../components/Button";
import { useUnsavedChanges } from "../../components/useUnsavedChanges";
import { useI18n } from "../../i18n";
import { ExtensionHistory } from "../../components/extensions/ExtensionHistory";
import { NewMcpForm } from "../../components/extensions/NewMcpForm";
import { NewSkillForm } from "./NewSkillForm";
import { PortableImportForm } from "./PortableImportForm";
import { ProjectRegisterForm } from "./ProjectRegisterForm";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

type Props = { workspace: ExtensionWorkspace };

export function NewMcpDialog({ workspace: w }: Props) {
  return (
      <NewMcpForm
        busy={w.writeBlocked}
        marketReturnFocus={w.nav.view === "market" ? w.nav.marketButton : undefined}
        registryEntry={w.nav.dialog?.type === "newMcp" ? w.nav.dialog.registryEntry : undefined}
        existingNames={w.items.filter((item) => item.kind === "mcp").map((item) => item.name)}
        onCancel={w.nav.closeDialog}
        onPutSecret={w.ext.putSecret}
        onSave={w.createMcp}
      />
  );
}

export function NewSkillDialog({ workspace: w }: Props) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const id = useId();
  const leave = useUnsavedChanges(Boolean(name || description), w.busy);
  return (
    <AppDialog title={t("extensions.toolbar.newSkill")} busy={w.busy} onClose={() => leave.request(w.nav.closeDialog)}
      footer={<><Button variant="secondary" disabled={w.busy} onClick={() => leave.request(w.nav.closeDialog)}>{t("confirm.cancel")}</Button>
        <Button type="submit" form={id} variant="primary" disabled={w.writeBlocked}>{t("extensions.newSkill.create")}</Button></>}>
      <NewSkillForm
        formId={id}
        busy={w.writeBlocked}
        newSkillName={name}
        setNewSkillName={setName}
        newSkillDescription={description}
        setNewSkillDescription={setDescription}
        createSkill={async () => {
          const definition = await w.ext.createSkill({ name: name.trim(), description: description.trim() });
          if (definition) w.nav.openSkillEditor(definition.id);
        }}
      />{leave.confirmation}
    </AppDialog>
  );
}

export function PortableImportDialog({ workspace: w }: Props) {
  const { t } = useI18n();
  const [path, setPath] = useState("");
  const id = useId();
  const leave = useUnsavedChanges(Boolean(path), w.busy);
  return (
    <AppDialog title={t("extensions.toolbar.importPortable")} busy={w.busy} onClose={() => leave.request(w.nav.closeDialog)}
      footer={<><Button variant="secondary" disabled={w.busy} onClick={() => leave.request(w.nav.closeDialog)}>{t("confirm.cancel")}</Button>
        <Button type="submit" form={id} variant="primary" disabled={w.writeBlocked}>{t("extensions.import.submit")}</Button></>}>
      <PortableImportForm
        formId={id}
        busy={w.writeBlocked}
        portablePath={path}
        setPortablePath={setPath}
        browsePortable={async () => {
          const picked = await pickFile("Portable package", ["json"]);
          if (picked) setPath(picked);
        }}
        submitPortableImport={async () => {
          if (!path.trim()) return;
          const result = await w.ext.importPortable(path.trim());
          if (result) w.nav.closeDialog();
        }}
      />{leave.confirmation}
    </AppDialog>
  );
}

export function ProjectDialog({ workspace: w }: Props) {
  const { t } = useI18n();
  const [root, setRoot] = useState("");
  const id = useId();
  const leave = useUnsavedChanges(Boolean(root), w.busy);
  return (
    <AppDialog title={t("extensions.toolbar.registerProject")} busy={w.busy} onClose={() => leave.request(w.nav.closeDialog)}
      footer={<><Button variant="secondary" disabled={w.busy} onClick={() => leave.request(w.nav.closeDialog)}>{t("confirm.cancel")}</Button>
        <Button type="submit" form={id} variant="primary" disabled={w.writeBlocked}>{t("extensions.project.submit")}</Button></>}>
      <ProjectRegisterForm
        formId={id}
        busy={w.writeBlocked}
        projectRoot={root}
        setProjectRoot={setRoot}
        browseProject={async () => {
          const picked = await pickDirectory();
          if (picked) setRoot(picked);
        }}
        submitProject={async () => {
          if (root.trim() && (await w.ext.addProject(root.trim()))) w.nav.closeDialog();
        }}
      />{leave.confirmation}
    </AppDialog>
  );
}

export function HistoryDialog({ workspace: w }: Props) {
  const { t } = useI18n();
  return (
    <AppDialog title={t("extensions.toolbar.history")} busy={w.busy} onClose={w.nav.closeDialog} wide>
      <ExtensionHistory
        records={w.ext.workspace?.history ?? []}
        items={w.items}
        busy={w.writeBlocked}
        projectNames={w.projectNames}
        onRestore={(id) => void w.applies.restore(id)}
      />
    </AppDialog>
  );
}
