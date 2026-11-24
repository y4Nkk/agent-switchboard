import { Button } from "../../components/Button";
import { AppDialog, AppDialogSuspension } from "../../components/AppDialog";
import {
  ExtensionPlanSheet,
  ExtensionRemoveSheet,
  SkillDisableScopeSheet,
} from "../../components/extensions/ExtensionPlanSheet";
import { useI18n } from "../../i18n";
import { ExtensionManagementDialog } from "./ExtensionManagementDialog";
import { McpEditorDialog, SkillEditorDialog } from "./ExtensionEditorDialogs";
import { McpRegistryUpdateSheet } from "./McpRegistryUpdateSheet";
import {
  HistoryDialog,
  NewMcpDialog,
  NewSkillDialog,
  PortableImportDialog,
  ProjectDialog,
} from "./ExtensionUtilityDialogs";
import { PortableExportSheet } from "./PortableExportSheet";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

function WorkspaceDialog({ workspace: w, suspended }: { workspace: ExtensionWorkspace; suspended: boolean }) {
  const { t } = useI18n();
  const dialog = w.nav.dialog;
  if (!dialog) return null;
  const content = (() => {
    switch (dialog.type) {
    case "newMcp":
      return <NewMcpDialog workspace={w} />;
    case "mcpRegistryUpdate":
      return <McpRegistryUpdateSheet workspace={w} report={dialog.report} />;
    case "newSkill":
      return <NewSkillDialog workspace={w} />;
    case "portableImport":
      return <PortableImportDialog workspace={w} />;
    case "project":
      return <ProjectDialog workspace={w} />;
    case "history":
      return <HistoryDialog workspace={w} />;
    case "export":
      return <PortableExportSheet workspace={w} item={dialog.item} />;
    case "mcpEditor":
      return <McpEditorDialog workspace={w} envelope={dialog.envelope} />;
    case "remove":
      return (
        <ExtensionRemoveSheet
          item={dialog.item}
          busy={w.busy}
          onCancel={w.nav.closeDialog}
          onConfirm={() => void w.deleteDefinition(dialog.item)}
        />
      );
    case "management":
    case "skillEditor": {
      const item = w.items.find((entry) => entry.id === dialog.definitionId);
      if (!item)
        return (
          <AppDialog title={t("extensions.workspace.readTitle")} busy={w.busy} onClose={w.nav.closeDialog}>
            <p>{t("extensions.workspace.readFailed")}</p>
            <Button variant="secondary" onClick={() => void w.ext.runExclusive(w.ext.refresh)}>
              {t("extensions.library.reload")}
            </Button>
          </AppDialog>
        );
      return dialog.type === "skillEditor" && item.kind === "skill" ? (
        <SkillEditorDialog key={item.id} workspace={w} item={item} />
      ) : (
        <ExtensionManagementDialog key={item.id} workspace={w} item={item} />
      );
    }
    }
  })();
  return (
    <AppDialogSuspension suspended={suspended}>
      {content}
    </AppDialogSuspension>
  );
}

export function ExtensionWorkspaceDialogs({ workspace: w }: { workspace: ExtensionWorkspace }) {
  const hasApplyPrompt = Boolean(w.applies.pendingWrite || w.applies.pendingDisable);
  return (
    <>
      <WorkspaceDialog workspace={w} suspended={hasApplyPrompt} />
      {w.applies.pendingWrite && (
        <ExtensionPlanSheet
          view={w.applies.pendingWrite.plan}
          busy={w.applies.confirmationBusy}
          projectNames={w.projectNames}
          resourceNames={w.resourceNames}
          onConfirm={w.applies.confirmPendingWrite}
          onCancel={w.applies.cancelPendingWrite}
        />
      )}
      {w.applies.pendingDisable && (
        <SkillDisableScopeSheet
          sharedSettings={w.applies.sharedSettings}
          busy={w.applies.confirmationBusy}
          onSharedSettingsChange={w.applies.setSharedSettings}
          onConfirm={() => void w.applies.confirmDisableScope()}
          onCancel={w.applies.cancelDisableScope}
        />
      )}
    </>
  );
}
