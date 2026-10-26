import { useEffect, useState } from "react";
import { SessionManager } from "../components/SessionManager";
import { ScrollArea } from "../components/ScrollArea";
import { useI18n } from "../i18n";
import { pageLabelKey, type Page } from "./navigation";
import { ClientConfigurationWorkspace } from "./ClientConfigurationWorkspace";
import { ExtensionsWorkspace } from "./ExtensionsWorkspace";
import { ProvidersWorkspace } from "./ProvidersWorkspace";
import { SettingsWorkspace } from "./SettingsWorkspace";
import { UsageWorkspace } from "./UsageWorkspace";
import type { SwitchboardModel } from "./useSwitchboardModel";

export function AppWorkspace({ model }: { model: SwitchboardModel }) {
  const { page } = model;
  const { t } = useI18n();
  const pageName = t(pageLabelKey(page));
  const [opened, setOpened] = useState<ReadonlySet<Page>>(() => new Set([page]));
  useEffect(() => {
    setOpened((current) => current.has(page) ? current : new Set([...current, page]));
  }, [page]);
  const visited = (target: Page) => page === target || opened.has(target);
  if (!model.navigationReady) return (
    <ScrollArea as="main" className="asb-main" aria-label={t("shell.workspaceRestoring.aria")}>
      <div className="asb-settings-skeleton" role="status" aria-label={t("shell.startupPageReading")}>
        <div className="asb-skeleton" /><div className="asb-skeleton" /><div className="asb-skeleton" />
      </div>
    </ScrollArea>
  );
  return (
    <ScrollArea as="main" className="asb-main" aria-label={pageName}>
      <div hidden={page !== "providers"} className="asb-page-stack">
        <ProvidersWorkspace model={model} active={page === "providers"} />
      </div>
      {visited("clientConfiguration") && <div hidden={page !== "clientConfiguration"} className="asb-page-stack">
        <ClientConfigurationWorkspace model={model} active={page === "clientConfiguration"} />
      </div>}
      {visited("extensions") && <div hidden={page !== "extensions"} className="asb-page-stack">
        <ExtensionsWorkspace model={model} />
      </div>}
      {visited("sessions") && <section className="asb-panel" hidden={page !== "sessions"} aria-label={t("shell.sessions.aria")}>
        <SessionManager active={page === "sessions"} />
      </section>}
      {visited("usage") && <div hidden={page !== "usage"} className="asb-page-stack">
        <UsageWorkspace active={page === "usage"} section={model.usageSection} onSectionChange={model.setUsageSection}
          probe={model.probe} claudeRecords={model.snapshot.records} codexRecords={model.snapshot.codexRecords} />
      </div>}
      {visited("settings") && <div hidden={page !== "settings"} className="asb-page-stack">
        <SettingsWorkspace model={model} active={page === "settings"} />
      </div>}
    </ScrollArea>
  );
}
