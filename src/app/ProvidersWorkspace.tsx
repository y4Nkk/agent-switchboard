import { useEffect } from "react";
import { useI18n } from "../i18n";
import { CodexProvidersPage } from "../pages/CodexProvidersPage";
import { ProviderImportPage } from "../pages/ProviderImportPage";
import { ProviderConversionPicker } from "../components/provider-editor/ProviderConversionPicker";
import { ProviderConversionBatchView } from "../components/provider-editor/ProviderConversionBatchView";
import { ProvidersPage } from "../pages/ProvidersPage";
import { useClientRouteTransition } from "./useClientRouteTransition";
import { useOnReturnToRoot } from "./PageRoot";
import type { SwitchboardModel } from "./useSwitchboardModel";

export function ProvidersWorkspace({ model, active }: { model: SwitchboardModel; active: boolean }) {
  const { t } = useI18n();
  const importing = model.providerView.kind === "import";
  const { snapshot, appFilter, activeProfileId, providers, providerSwitch, appSettingsState, busy, operations } = model;
  const { discoveryState, ccImport, sqlImport } = model;
  const { route, userConfigRoute, codexEditorSession, claudeEditorSession } = useProvidersWorkspaceRoute(model, active);
  const subpageOpen = importing || (appFilter === "claude" && model.providerView.kind === "usage")
    || codexEditorSession !== null || claudeEditorSession !== null;
  useOnReturnToRoot(active && subpageOpen, () => {
    model.setProviderView({ kind: "list" });
    providers.closeEditorAtList();
  }, importing && busy);
  const editorNavigation = { request: providers.editorRequest, onRequestHandled: providers.handleEditorRequest,
    onReplaceSession: providers.replaceEditorSession, onReload: providers.reloadEditor, inventory: { codex: snapshot.codexRecords,
      profiles: [...snapshot.records, ...snapshot.codexOfficialRecords], activeIds: { codex: activeProfileId("codex"), claude: activeProfileId("claude") } } };
  return (
    <div ref={route.workspaceRef} className="asb-client-route-workspace">
      <ProviderConversionFlow model={model} active={active && !importing} />
      {providers.pendingReleaseError != null && <p className="asb-field-help" role="status">{t("operations.save.retryRelease")}</p>}
      {importing && <div className="asb-subpage-route" hidden={!active}>
        <ProviderImportPage appFilter={appFilter} discovery={discoveryState.discovery} busy={busy}
          ccScan={ccImport.ccScan} ccSelected={ccImport.ccSelected} ccResult={ccImport.ccResult} ccDirectory={ccImport.ccDirectory}
          sqlScan={sqlImport.sqlScan} sqlSelected={sqlImport.sqlSelected} sqlResult={sqlImport.sqlResult}
          onBack={() => model.setProviderView({ kind: "list" })}
          onScanLocal={() => void discoveryState.runDiscovery()} onImportLocal={discoveryState.runImport}
          onScanCc={() => void ccImport.runCcScan()} onImportCc={ccImport.runCcImport} onCcDirectory={ccImport.changeCcDirectory}
          onSelectCc={(key, checked) => ccImport.setCcSelected((current) => ({ ...current, [key]: checked }))}
          onApplySql={(path) => void sqlImport.runSqlApply(path)} onImportSql={sqlImport.runSqlImport}
          onSelectSql={(key, checked) => sqlImport.setSqlSelected((current) => ({ ...current, [key]: checked }))}
          onError={model.reportError} />
      </div>}
      <CodexProvidersPage
        active={active && !importing && appFilter === "codex"} leaving={route.leavingApp === "codex"} routeContentRef={route.contentRefs.codex}
        onSelectApp={route.selectApp} records={snapshot.codexRecords}
        officialRecord={snapshot.codexOfficialRecords[0] ?? null}
        activeProfileId={activeProfileId("codex")}
        busy={busy || !!providers.pendingSave} onBusy={model.setBusy} onError={model.reportError} onRefresh={async () => { await snapshot.refresh(); }}
        onImport={() => { providerSwitch.clearCandidates(); model.setProviderView({ kind: "import" }); }}
        onDelete={(record) => providers.setDeletePending({ kind: "codexThirdParty", record })}
        onDeleteOfficial={(record) => providers.setDeletePending({ kind: "generic", profile: record.profile })}
        onConvert={providers.openConversion}
        editorNavigation={editorNavigation} editorSession={codexEditorSession} onNew={providers.newEditor} onEdit={providers.openCodexEditor}
        onRoutes={(id) => providers.openRoutes("codex", id)}
        onEditOfficial={providers.openCodexOfficialEditor} onCloseEditor={providers.closeEditor}
        onSave={providers.saveCodexProfile} onSaveOfficial={providers.saveGenericProfile}
        onSwitchAccessMode={providers.switchCodexAccessMode}
        onSaveOfficialQuotaInterval={providers.saveOfficialQuotaInterval}
        expandedUsageIds={appSettingsState.appSettings?.expandedUsageIds ?? []}
        onToggleUsage={(profileId) => appSettingsState.toggleUsageExpanded(profileId)}
        onSaveUsageQuery={providers.saveCodexProfileUsageQuery}
        loginBlocker={snapshot.loginBlocker} statuses={snapshot.statuses} relayProfiles={snapshot.relayProfiles} locks={snapshot.locks}
        userConfigModel={userConfigRoute?.model ?? null} userConfigWarnings={userConfigRoute?.scopeWarnings ?? []} />
    <ProvidersPage active={active && !importing && appFilter === "claude"} leaving={route.leavingApp === "claude"} routeContentRef={route.contentRefs.claude}
      onSelectApp={route.selectApp} view={model.providerView} onViewChange={model.setProviderView}
      profiles={snapshot.profiles} relayProfiles={snapshot.relayProfiles}
      activeProfileId={activeProfileId("claude")} statuses={snapshot.statuses} locks={snapshot.locks}
      userConfigModel={userConfigRoute?.model ?? null} userConfigWarnings={userConfigRoute?.scopeWarnings ?? []}
      editorNavigation={editorNavigation} editorSession={claudeEditorSession} onNew={providers.newEditor}
      busy={busy || !!providers.pendingSave} expandedUsageIds={appSettingsState.appSettings?.expandedUsageIds ?? []}
      activationCandidate={providerSwitch.activationCandidate}
      onImport={() => { providerSwitch.clearCandidates(); model.setProviderView({ kind: "import" }); }}
      onCloseEditor={providers.closeEditor} onSave={providers.saveGenericProfile}

      onSaveUsageQuery={async (profile, query) => {
        const saved = await providers.saveProfileUsageQuery(profile, query);
        if (saved) {
          providers.selectApp(profile.app);
          providerSwitch.setTargetProfile(profile.id);
        }
        return saved;
      }}
      onReorder={providers.dragReorderClaudeProfiles}
      onToggleUsage={(profile) => appSettingsState.toggleUsageExpanded(profile.id)}
      onActivate={providerSwitch.requestActivation}
      onConfirmSwitch={() => void operations.runSwitch()}
      onCancelActivation={providerSwitch.clearCandidates}
      onEdit={providers.openEditor}
      onConvert={providers.openConversion}
      onRoutes={(profile) => providers.openRoutes("claude", profile.id)}
      onDelete={(profile) => providers.setDeletePending({ kind: "generic", profile })}
      onRefresh={async () => { await snapshot.refresh(); }} />
    </div>
  );
}

function ProviderConversionFlow({ model, active }: { model: SwitchboardModel; active: boolean }) {
  const { providers, appFilter, busy } = model;
  if (!active) return null;
  return <>
    {providers.conversionSelection?.sourceApp === appFilter && <ProviderConversionPicker app={appFilter}
      sources={providers.conversionSelection.sources} busy={busy} onStart={providers.startConversion} onClose={providers.endConversion} />}
    {providers.conversionBatch && <ProviderConversionBatchView batch={providers.conversionBatch}
      busy={busy} onSkip={providers.skipConversion} onRetry={providers.retryConversion} onEnd={providers.endConversion} />}
  </>;
}

function useProvidersWorkspaceRoute(model: SwitchboardModel, active: boolean) {
  const { appFilter, providers, snapshot, providerSwitch } = model;
  const importing = model.providerView.kind === "import";
  const route = useClientRouteTransition({
    app: appFilter,
    active: active && model.providerView.kind === "list" && !providers.editorSession,
    onSelectApp: providers.selectApp,
  });
  const editorApp = providers.editorSession?.app ?? appFilter;
  const userConfigRoute = snapshot.statuses?.find((status) => status.app === editorApp)?.route ?? null;
  const codexEditorSession = providers.editorSession?.app === "codex" ? providers.editorSession : null;
  const claudeEditorSession = providers.editorSession?.app === "claude" ? providers.editorSession : null;
  useEffect(() => {
    if (!active || importing || appFilter !== "claude" || claudeEditorSession) providerSwitch.clearCandidates();
  }, [active, importing, appFilter, claudeEditorSession, providerSwitch.clearCandidates]);
  return { route, userConfigRoute, codexEditorSession, claudeEditorSession };
}
