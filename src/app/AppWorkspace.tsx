import { useEffect, useState, type ReactNode } from "react";
import { SessionManager } from "../components/SessionManager";
import { ScrollArea } from "../components/ScrollArea";
import { useI18n } from "../i18n";
import { pageLabelKey, type Page } from "./navigation";
import { ClientConfigurationWorkspace } from "./ClientConfigurationWorkspace";
import { ExtensionsWorkspace } from "./ExtensionsWorkspace";
import { GatewayWorkspace } from "./GatewayWorkspace";
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
        <ConfigurationSurface ready={model.preparation.configurationReady}><ProvidersWorkspace model={model} active={page === "providers"} /></ConfigurationSurface>
      </div>
      {visited("gateway") && <div hidden={page !== "gateway"} className="asb-page-stack">
        <ConfigurationSurface ready={model.preparation.configurationReady}><GatewayWorkspace model={model} active={page === "gateway"} /></ConfigurationSurface>
      </div>}
      {visited("clientConfiguration") && <div hidden={page !== "clientConfiguration"} className="asb-page-stack">
        <ConfigurationSurface ready={model.preparation.configurationReady}><ClientConfigurationWorkspace model={model} active={page === "clientConfiguration"} /></ConfigurationSurface>
      </div>}
      {visited("extensions") && <div hidden={page !== "extensions"} className="asb-page-stack">
        <ExtensionsWorkspace model={model} active={page === "extensions"} />
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

function ConfigurationSurface({ ready, children }: { ready: boolean; children: ReactNode }) {
  const { t } = useI18n();
  return <>
    {!ready && <p className="asb-field-error" role="alert">{t("preparation.configurationUnavailable")}</p>}
    <div className="asb-page-stack" inert={!ready} aria-disabled={!ready}>{children}</div>
  </>;
}
