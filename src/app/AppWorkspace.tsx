import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import { ScrollArea } from "../components/ScrollArea";
import { PageErrorBoundary } from "../components/PageErrorBoundary";
import { useI18n } from "../i18n";
import { pageLabelKey, type Page } from "./navigation";
import { PageRoot } from "./PageRoot";
import type { SwitchboardModel } from "./useSwitchboardModel";

const ProvidersWorkspace = lazy(() => import("./ProvidersWorkspace").then(({ ProvidersWorkspace }) => ({ default: ProvidersWorkspace })));
const GatewayWorkspace = lazy(() => import("./GatewayWorkspace").then(({ GatewayWorkspace }) => ({ default: GatewayWorkspace })));
const ClientConfigurationWorkspace = lazy(() => import("./ClientConfigurationWorkspace").then(({ ClientConfigurationWorkspace }) => ({ default: ClientConfigurationWorkspace })));
const ExtensionsWorkspace = lazy(() => import("./ExtensionsWorkspace").then(({ ExtensionsWorkspace }) => ({ default: ExtensionsWorkspace })));
const SessionManager = lazy(() => import("../components/SessionManager").then(({ SessionManager }) => ({ default: SessionManager })));
const UsageWorkspace = lazy(() => import("./UsageWorkspace").then(({ UsageWorkspace }) => ({ default: UsageWorkspace })));
const SettingsWorkspace = lazy(() => import("./SettingsWorkspace").then(({ SettingsWorkspace }) => ({ default: SettingsWorkspace })));

export function AppWorkspace({ model }: { model: SwitchboardModel }) {
  const { page } = model;
  const { t } = useI18n();
  const pageName = t(pageLabelKey(page));
  const [opened, setOpened] = useState<ReadonlySet<Page>>(() => new Set());
  const { navigationReady } = model;
  useEffect(() => {
    if (!navigationReady) return;
    setOpened((current) => current.has(page) ? current : new Set([...current, page]));
  }, [navigationReady, page]);
  const visited = (target: Page) => page === target || opened.has(target);
  if (!navigationReady) return (
    <ScrollArea as="main" className="asb-main" aria-label={t("shell.workspaceRestoring.aria")}>
      <WorkspaceLoading label={t("shell.startupPageReading")} />
    </ScrollArea>
  );
  return (
    <ScrollArea as="main" className="asb-main" aria-label={pageName}>
      <PageRoot page="providers" request={model.pageRoot}>{visited("providers") && <div hidden={page !== "providers"} className="asb-page-stack">
        <WorkspaceBoundary active={page === "providers"}>
          <ConfigurationSurface ready={model.preparation.configurationReady}><ProvidersWorkspace model={model} active={page === "providers"} /></ConfigurationSurface>
        </WorkspaceBoundary>
      </div>}</PageRoot>
      <PageRoot page="gateway" request={model.pageRoot}>{visited("gateway") && <div hidden={page !== "gateway"} className="asb-page-stack">
        <WorkspaceBoundary active={page === "gateway"}>
          <ConfigurationSurface ready={model.preparation.configurationReady}><GatewayWorkspace model={model} active={page === "gateway"} /></ConfigurationSurface>
        </WorkspaceBoundary>
      </div>}</PageRoot>
      <PageRoot page="clientConfiguration" request={model.pageRoot}>{visited("clientConfiguration") && <div hidden={page !== "clientConfiguration"} className="asb-page-stack">
        <WorkspaceBoundary active={page === "clientConfiguration"}>
          <ConfigurationSurface ready={model.preparation.configurationReady}><ClientConfigurationWorkspace model={model} active={page === "clientConfiguration"} /></ConfigurationSurface>
        </WorkspaceBoundary>
      </div>}</PageRoot>
      <PageRoot page="extensions" request={model.pageRoot}>{visited("extensions") && <div hidden={page !== "extensions"} className="asb-page-stack">
        <WorkspaceBoundary active={page === "extensions"}>
          <ExtensionsWorkspace model={model} active={page === "extensions"} />
        </WorkspaceBoundary>
      </div>}</PageRoot>
      <PageRoot page="sessions" request={model.pageRoot}>{visited("sessions") && <section className="asb-panel" hidden={page !== "sessions"} aria-label={t("shell.sessions.aria")}>
        <WorkspaceBoundary active={page === "sessions"}>
          <SessionManager active={page === "sessions"} />
        </WorkspaceBoundary>
      </section>}</PageRoot>
      <PageRoot page="usage" request={model.pageRoot}>{visited("usage") && <div hidden={page !== "usage"} className="asb-page-stack">
        <WorkspaceBoundary active={page === "usage"}>
          <UsageWorkspace active={page === "usage"} section={model.usageSection} onSectionChange={model.setUsageSection}
            probe={model.probe} claudeRecords={model.snapshot.records} codexRecords={model.snapshot.codexRecords} />
        </WorkspaceBoundary>
      </div>}</PageRoot>
      <PageRoot page="settings" request={model.pageRoot}>{visited("settings") && <div hidden={page !== "settings"} className="asb-page-stack">
        <WorkspaceBoundary active={page === "settings"}>
          <SettingsWorkspace model={model} active={page === "settings"} />
        </WorkspaceBoundary>
      </div>}</PageRoot>
    </ScrollArea>
  );
}

function WorkspaceLoading({ label }: { label: string }) {
  return <div className="asb-settings-skeleton" role="status" aria-label={label}>
    <div className="asb-skeleton" /><div className="asb-skeleton" /><div className="asb-skeleton" />
  </div>;
}

function WorkspaceBoundary({ active, children }: { active: boolean; children: ReactNode }) {
  const { t } = useI18n();
  return <PageErrorBoundary active={active}>
    <Suspense fallback={<WorkspaceLoading label={t("shell.workspaceLoading")} />}>
      {children}
      {active && <FirstWorkspacePaint />}
    </Suspense>
  </PageErrorBoundary>;
}

function FirstWorkspacePaint() {
  useEffect(() => {
    const name = "desktop:first-workspace-paint";
    if (performance.getEntriesByName(name, "measure").length > 0) return;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => performance.measure(name, { start: 0 }));
    });
    return () => cancelAnimationFrame(frame);
  }, []);
  return null;
}

function ConfigurationSurface({ ready, children }: { ready: boolean; children: ReactNode }) {
  const { t } = useI18n();
  return <>
    {!ready && <p className="asb-field-error" role="alert">{t("preparation.configurationUnavailable")}</p>}
    <div className="asb-page-stack" inert={!ready} aria-disabled={!ready}>{children}</div>
  </>;
}
