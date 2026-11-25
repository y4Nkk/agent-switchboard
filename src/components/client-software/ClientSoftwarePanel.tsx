import { useId, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppKind } from "../../api/shared";
import { useWorkspaceVisible } from "../../hooks/use-workspace-visible";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { CollapsibleSection } from "../Collapsible";
import { ClientManagementModule } from "../client-management/ClientManagementModule";
import { SoftwareArt } from "./SoftwareArt";
import { SoftwareCard } from "./SoftwareCard";
import { UpdateReview } from "./UpdateReview";
import { UpdateTask } from "./UpdateTask";
import { useClientSoftware } from "./useClientSoftware";

/** Reserves the featured card's footprint until the scan returns. */
function SoftwareSkeleton() {
  const { t } = useI18n();
  return <div className="asb-tool-stage">
    <div className="asb-software-skeleton" role="status" aria-label={t("software.loading")}>
      <div className="asb-skeleton" aria-hidden="true" />
      <div className="asb-software-skeleton-lines" aria-hidden="true">
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
      </div>
    </div>
  </div>;
}

export function ClientSoftwarePanel({ client, active }: { client: AppKind; active: boolean }) {
  const { t } = useI18n();
  const othersId = useId();
  const visible = useWorkspaceVisible(active);
  const software = useClientSoftware(visible);
  const [issues, setIssues] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [linkError, setLinkError] = useMessageState();
  const items = software.snapshot?.installations.filter((item) => item.app === client) ?? [];
  const featured = items.find((item) => item.active) ?? null;
  const others = items.filter((item) => item !== featured);
  const disabled = software.busy || software.updating;
  const empty = software.snapshot !== null && items.length === 0 && !software.updating;
  const discoveryIssues = software.snapshot?.issues ?? [];
  const url = client === "codex" ? "https://developers.openai.com/codex/cli" : "https://code.claude.com/docs/en/setup";
  const selectLocation = <Button variant="secondary" size="compact" disabled={disabled}
    onClick={() => void software.select(client)}>{t("software.select")}</Button>;
  return <ClientManagementModule title={t("software.title")} refreshLabel={t("software.refresh")}
    busy={software.busy} refreshDisabled={disabled} onRefresh={() => void software.refresh()}>
    {software.error && <p role="alert" className="asb-field-error">{software.error}</p>}
    {linkError && <p role="alert" className="asb-field-error">{linkError}</p>}
    {!software.snapshot && software.busy && <SoftwareSkeleton />}
    {empty && <div className="asb-tool-stage asb-software-empty">
      <SoftwareArt form="cli" />
      <div>
        <p>{t(discoveryIssues.length ? "software.discoveryIncomplete" : "software.notInstalled")}</p>
        <div className="asb-software-actions">
          {selectLocation}
          <Button variant="secondary" size="compact"
            onClick={() => void openUrl(url).catch(setLinkError)}>{t("software.installGuide")}</Button>
        </div>
      </div>
    </div>}
    {featured && <div className="asb-tool-stage">
      <SoftwareCard key={featured.id} item={featured} featured disabled={disabled} expanded={expandedId === featured.id}
        onExpandedChange={(open) => setExpandedId(open ? featured.id : null)} onUpdate={(id) => void software.prepare(id)} />
    </div>}
    {software.task && <UpdateTask key={software.task.id} task={software.task} busy={software.busy} onCancel={() => void software.cancel()} />}
    {others.length > 0 && <section className="asb-software-others" aria-labelledby={othersId}>
      <h4 className="asb-group-title" id={othersId}>{t("software.otherInstallations")}</h4>
      <div className="asb-software-grid">
        {others.map((item) => <SoftwareCard key={item.id} item={item} disabled={disabled} expanded={expandedId === item.id}
          onExpandedChange={(open) => setExpandedId(open ? item.id : null)} onUpdate={(id) => void software.prepare(id)} />)}
      </div>
    </section>}
    {(!empty || discoveryIssues.length > 0) && <div className="asb-software-foot-module">
      {discoveryIssues.length > 0 && <CollapsibleSection quiet open={issues} onOpenChange={setIssues} label={t("software.issues")}>
        <ul className="asb-software-issues">{discoveryIssues.map((issue, index) => <li key={index}>{issue}</li>)}</ul>
      </CollapsibleSection>}
      {!empty && selectLocation}
    </div>}
    {active && software.plan && <UpdateReview plan={software.plan} busy={software.busy} error={software.error}
      onClose={software.dismissPlan} onStart={() => void software.start()} />}
  </ClientManagementModule>;
}
