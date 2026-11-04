import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppKind } from "../../api/shared";
import { useWorkspaceVisible } from "../../hooks/use-workspace-visible";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { CollapsibleSection } from "../Collapsible";
import { ClientManagementModule } from "../client-management/ClientManagementModule";
import { SoftwareCard } from "./SoftwareCard";
import { UpdateReview } from "./UpdateReview";
import { UpdateTask } from "./UpdateTask";
import { useClientSoftware } from "./useClientSoftware";

export function ClientSoftwarePanel({ client, active }: { client: AppKind; active: boolean }) {
  const { t } = useI18n();
  const visible = useWorkspaceVisible(active);
  const software = useClientSoftware(visible);
  const [issues, setIssues] = useState(false);
  const [linkError, setLinkError] = useMessageState();
  const items = software.snapshot?.installations.filter((item) => item.app === client) ?? [];
  const url = client === "codex" ? "https://developers.openai.com/codex/cli" : "https://code.claude.com/docs/en/setup";
  return <ClientManagementModule title={t("software.title")} description={t("software.description")}
    refreshLabel={t("software.refresh")} busy={software.busy} refreshDisabled={software.busy || software.updating}
    onRefresh={() => void software.refresh()}>
    {software.error && <p role="alert" className="asb-field-error">{software.error}</p>}
    {linkError && <p role="alert" className="asb-field-error">{linkError}</p>}
    {!software.snapshot && software.busy && <p role="status">{t("software.loading")}</p>}
    {software.snapshot && items.length === 0 && !software.updating && <div className="asb-software-details">
      <p className="asb-scope-note">{t(software.snapshot.issues.length ? "software.discoveryIncomplete" : "software.notInstalled")}</p>
      <Button variant="secondary" size="compact" onClick={() => void openUrl(url).catch(setLinkError)}>{t("software.installGuide")}</Button>
    </div>}
    {software.task && <UpdateTask task={software.task} busy={software.busy} onCancel={() => void software.cancel()} />}
    {items.map((item) => <SoftwareCard key={item.id} item={item} disabled={software.busy || software.updating}
      onUpdate={() => void software.prepare(item.id)} />)}
    {!!software.snapshot?.issues.length && <CollapsibleSection quiet open={issues} onOpenChange={setIssues} label={t("software.issues")}>
      <ul className="asb-software-issues">{software.snapshot.issues.map((issue, index) => <li key={index}>{issue}</li>)}</ul>
    </CollapsibleSection>}
    <p className="asb-scope-note">{t("software.scope")}</p>
    <Button variant="ghost" size="compact" disabled={software.busy || software.updating} onClick={() => void software.select(client)}>{t("software.select")}</Button>
    {active && software.plan && <UpdateReview plan={software.plan} busy={software.busy} error={software.error}
      onClose={software.dismissPlan} onStart={() => void software.start()} />}
  </ClientManagementModule>;
}
