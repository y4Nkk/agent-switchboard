import type { McpRegistryUpdate } from "../../api/extensions/registry";
import { ConfirmSheet } from "../../components/ConfirmSheet";
import { FactList } from "../../components/FactList";
import { useI18n } from "../../i18n";
import { deployUpdatedDefinition } from "./ExtensionEditorDialogs";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

export function McpRegistryUpdateSheet({ workspace: w, report }: { workspace: ExtensionWorkspace; report: McpRegistryUpdate }) {
  const { t } = useI18n();
  const item = w.items.find((entry) => entry.id === report.definitionId);
  const previous = item?.mcpMetadata?.registry;
  const stale = !previous || item?.revision !== report.revision;
  const associate = report.action === "associateCurrent";
  const confirm = async () => {
    if (stale) return;
    const saved = await w.registryUpdates.apply(report);
    if (!saved) return;
    w.nav.closeDialog();
    if (saved.kind === "endpointReplaced") await deployUpdatedDefinition(w, saved.preparation);
  };
  return <ConfirmSheet title={t(associate ? "extensions.registry.associateTitle" : "extensions.registry.updateTitle", { name: item?.name ?? report.source.name })}
    confirmLabel={t(associate ? "extensions.registry.associateVersion" : "extensions.registry.confirmUpdate")} busy={w.busy} confirmDisabled={stale || w.writeBlocked}
    onCancel={w.nav.closeDialog} onConfirm={() => void confirm()}>
    {stale ? <p role="alert">{t("extensions.registry.updateStale")}</p> : <>
      <p>{previous.version === report.source.version
        ? t("extensions.registry.sourceVersion", { name: report.source.name, version: report.source.version })
        : t("extensions.registry.versionChange", { from: previous.version, to: report.source.version })}</p>
      {associate ? <FactList rows={[{ label: t("extensions.registry.currentEndpoint"), value: report.source.endpoint }]} /> : <FactList rows={[
        { label: t("extensions.registry.currentEndpoint"), value: previous.endpoint },
        { label: t("extensions.registry.nextEndpoint"), value: report.source.endpoint },
      ]} />}
      <p>{t(associate ? "extensions.registry.associateEffect" : "extensions.registry.updateEffect")}</p>
    </>}
  </ConfirmSheet>;
}
