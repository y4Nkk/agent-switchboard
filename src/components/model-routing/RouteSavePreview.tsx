import { useState } from "react";
import { useI18n, type MessageKey } from "../../i18n";
import { CollapsibleSection } from "../Collapsible";
import { localizedMessageText } from "../../i18n/errors";
import { PreviewInspector } from "../PreviewInspector";
import { Table } from "../Table";
import type { PendingRouteSave } from "./useRouteApply";

export function RouteSavePreview({ pending }: { pending: PendingRouteSave }) {
  const { t } = useI18n();
  const [nativeOpen, setNativeOpen] = useState(false);
  const entry = pending.preview?.preview.changes.find(({ key }) => key.endsWith("base_url") || key === "env.ANTHROPIC_BASE_URL");
  const target = (value: PendingRouteSave["changes"][number]["after"]) => value.target
    ? `${value.target.provider ?? t("modelRoutes.unresolved")} / ${value.target.model || t("modelRoutes.clientDefault")}${value.oneM ? " [1M]" : ""} · ${t(value.mode === "provider" ? "modelRoutes.specify" : "modelRoutes.followPrimary")}`
    : t("modelRoutes.followPrimary");
  return <>
    <p>{t("modelRoutes.confirmHelp")}</p>
    <dl className="asb-model-route-summary">
      <dt>{t("modelRoutes.editing")}</dt><dd>{pending.document.draft.name}</dd>
      <dt>{t("modelRoutes.activeUnchanged")}</dt><dd>{pending.currentName ?? t("modelRoutes.unmatched")}</dd>
    </dl>
    <Table rows={pending.changes} rowKey={(row) => row.role} ariaLabel={t("modelRoutes.routeChanges")}
      columns={[
        { key: "role", header: t("modelRoutes.purpose"), render: (row) => t(`modelRoutes.role.${row.role}` as MessageKey) },
        { key: "before", header: t("providers.diff.column.before"), render: (row) => target(row.before) },
        { key: "after", header: t("providers.diff.column.after"), render: (row) => target(row.after) },
      ]} />
    <p className="asb-field-help">{t(pending.kind === "saveAndApply" ? "modelRoutes.liveEffect" : "modelRoutes.onlyEffect")}</p>
    {pending.preview && <>
      {pending.preview.preview.warnings.length > 0 && <ul className="asb-warnings" aria-label={t("providers.preview.warningsAria")}>
        {pending.preview.preview.warnings.map((warning, index) => <li key={`${warning.key}:${index}`}>{localizedMessageText(warning, t)}</li>)}
      </ul>}
      <p className="asb-field-help">{t("modelRoutes.entryHelp")}</p>
      {entry && <dl className="asb-model-route-summary">
        <dt>{t("modelRoutes.clientEntry")}</dt><dd><code>{entry.before ?? "—"}</code> → <code>{entry.after ?? "—"}</code></dd>
      </dl>}
      <CollapsibleSection open={nativeOpen} onOpenChange={setNativeOpen} label={t("modelRoutes.nativeChanges")}>
        <PreviewInspector filePreview={pending.preview} showUserConfigModel={false} showWarnings={false} userConfigModel={null} userConfigWarnings={[]} />
      </CollapsibleSection>
    </>}
  </>;
}
