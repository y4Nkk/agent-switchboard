import { useState } from "react";
import type { AppKind } from "../api/client";
import { useI18n } from "../i18n";
import { ClientLogo } from "../components/ClientLogo";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { Tabs } from "../components/Tabs";
import { IntegrationPane } from "../components/claude-management/IntegrationPane";
import { useClaudeOperations } from "../components/claude-management/operations";
import { ProjectPlansPane } from "../components/codex-management/ProjectPlansPane";
import { useCodexOperations } from "../components/codex-management/operations";
import { ClientSoftwarePanel } from "../components/client-software/ClientSoftwarePanel";

interface ClientManagementPageProps {
  onChanged: () => void;
  active: boolean;
}

/** Only client-specific tools without another workspace owner belong here. */
export function ClientManagementPage({ onChanged, active }: ClientManagementPageProps) {
  const { t } = useI18n();
  const [client, setClient] = useState<AppKind>("codex");
  const codex = useCodexOperations(onChanged);
  const claude = useClaudeOperations(onChanged);
  const operations = client === "codex" ? codex : claude;
  const contentId = "client-management-content";

  return (
    <section className="asb-panel" aria-label={t("clientConfig.tools.title")}>
      <ModuleHeader
        title={t("clientConfig.tools.title")}
        primary={
          <Tabs scope="client-management-client" label={t("clientConfig.tools.clientLabel")} value={client} onChange={setClient}
            tabs={[
              {
                value: "codex",
                label: <span className="asb-client-management-tab-label"><ClientLogo app="codex" className="asb-client-management-tab-logo" />Codex</span>,
                controls: contentId,
                disabled: operations.busy,
              },
              {
                value: "claude",
                label: <span className="asb-client-management-tab-label"><ClientLogo app="claude" className="asb-client-management-tab-logo" />Claude</span>,
                controls: contentId,
                disabled: operations.busy,
              },
            ]} />
        }
      />
      <div className="asb-client-management-content" id={contentId} role="tabpanel" aria-labelledby={`client-management-client-${client}-tab`}>
        <ClientSoftwarePanel client={client} active={active} />
        {operations.error && <p role="alert" className="asb-field-error">{operations.error}</p>}
        {operations.notice && <p role="status" className="asb-scope-note">{operations.notice}</p>}
        <div hidden={client !== "codex"}><ProjectPlansPane operations={codex} /></div>
        <div hidden={client !== "claude"}><IntegrationPane operations={claude} /></div>
      </div>
    </section>
  );
}
