import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { SessionManager } from "./components/SessionManager";
import { Toaster } from "./components/Toaster";
import { AppShell } from "./app/AppShell";
import { useDevtoolsShortcut, useKeyboardFocusMarker } from "./app/global-effects";
import { OperationConfirmSheets } from "./app/OperationConfirmSheets";
import { useSwitchboardModel } from "./app/useSwitchboardModel";
import { BackupsPage } from "./pages/BackupsPage";
import { CommonSettingsPage } from "./pages/CommonSettingsPage";
import { CcImportSection, DiscoveryPage } from "./pages/DiscoveryPage";
import { LogsPage } from "./pages/LogsPage";
import { OverviewPage } from "./pages/OverviewPage";
import { ProvidersPage } from "./pages/ProvidersPage";
import { SettingsPage } from "./pages/SettingsPage";
import { UsagePage } from "./pages/UsagePage";

/** View composition root: the model lives in `useSwitchboardModel` (domain
 * hooks in `app/`), page rendering in `pages/`, and this file only wires
 * the two together. */
export default function App() {
  const {
    page,
    setPage,
    appFilter,
    busy,
    error,
    snapshot,
    activeProfileId,
    switchPreview,
    commonSettings,
    promptDocuments,
    appSettingsState,
    cloudBackup,
    updateCheck,
    discoveryState,
    ccImport,
    selectedProfile,
    operations,
    providers,
    lastSwitchOverall,
    openBackupFolder,
  } = useSwitchboardModel();
  const { preview } = switchPreview;
  const { editorMode, setEditorMode } = providers;
  const requestSwitch = () => operations.setConfirmingSwitch(true);
  // Read-only facts from the displayed client's user-level configuration file.
  // They deliberately do not claim to identify a running session's overrides.
  const userConfigRoute =
    snapshot.statuses?.find((status) => status.app === appFilter)?.route ?? null;
  const userConfigModel = userConfigRoute?.model ?? null;
  const userConfigWarnings = userConfigRoute?.scopeWarnings ?? [];
  useDevtoolsShortcut();
  useKeyboardFocusMarker();

  return (
    <>
      <AppShell
        page={page}
        onPageChange={setPage}
        error={error}
        busy={busy}
        onResetStore={() => providers.setResetStorePending(true)}
        settingsError={appSettingsState.loadError}
        onRepairSettings={() => void appSettingsState.repairSettings()}
        pin={appSettingsState.pin}
        update={
          updateCheck.updateCheck
            ? {
                latestVersion: updateCheck.updateCheck.latestVersion,
                onOpen: () => setPage("设置"),
              }
            : null
        }
      >
        <AppErrorBoundary>
          <main className="asb-main" aria-label={page}>
            {page === "概览" && (
              <OverviewPage
                statuses={snapshot.statuses}
                locks={snapshot.locks}
                busy={busy}
                relayHidden={editorMode !== null}
                onRefresh={() => void snapshot.refresh()}
                onRecoverLock={operations.setRecoverLockPending}
              />
            )}
            {page === "供应商" && (
              <ProvidersPage
                profiles={snapshot.profiles}
                appFilter={appFilter}
                activeProfileId={activeProfileId(appFilter)}
                userConfigModel={userConfigModel}
                userConfigWarnings={userConfigWarnings}
                selectedId={snapshot.selectedId}
                selectedProfile={selectedProfile}
                editorMode={editorMode}
                preview={preview}
                busy={busy}
                collapsedUsageIds={appSettingsState.appSettings?.collapsedUsageIds ?? []}
                onSelectApp={providers.selectApp}
                onNew={() => setEditorMode("new")}
                onCloseEditor={() => setEditorMode(null)}
                onSave={providers.saveProfile}
                onSaveUsageQuery={providers.saveProfileUsageQuery}
                onSelect={switchPreview.selectProfile}
                onReorder={providers.dragReorderProfiles}
                onToggleUsage={(profile) => appSettingsState.toggleUsageCollapsed(profile.id)}
                onActivate={switchPreview.previewProfile}
                onTogglePreview={switchPreview.togglePreviewProfile}
                onEdit={providers.openEditor}
                onDelete={providers.setDeletePending}
                onRequestSwitch={requestSwitch}
                onCancelPreview={switchPreview.retractPreview}
              />
            )}
            {page === "通用设置" && (
              <CommonSettingsPage
                app={commonSettings.settingsApp}
                onSelectApp={commonSettings.setSettingsApp}
                editorState={commonSettings.editorState}
                busy={busy}
                configStatus={snapshot.statuses?.find(
                  (status) => status.app === commonSettings.settingsApp,
                )}
                hasActiveProvider={
                  snapshot.activeProfileId(commonSettings.settingsApp) !== null
                }
                onValueChange={commonSettings.changeValue}
                onResetGroup={commonSettings.resetGroupToDefaults}
                onSave={(app) => void commonSettings.saveSettings(app)}
                onRetryLoad={commonSettings.retryLoad}
                onPreview={commonSettings.previewSettings}
                promptDocument={promptDocuments.documents[commonSettings.settingsApp]}
                promptDraft={
                  promptDocuments.drafts[commonSettings.settingsApp] ??
                  promptDocuments.documents[commonSettings.settingsApp]?.content ??
                  ""
                }
                promptDirty={promptDocuments.isPromptDirty(commonSettings.settingsApp)}
                onPromptDraftChange={(content) =>
                  promptDocuments.setPromptDraft(commonSettings.settingsApp, content)
                }
                onSavePrompt={() =>
                  void promptDocuments.savePromptDocument(commonSettings.settingsApp)
                }
                onDiscardPrompt={() =>
                  promptDocuments.discardPromptDraft(commonSettings.settingsApp)
                }
                onReloadPrompt={() =>
                  promptDocuments.reloadPromptDocument(commonSettings.settingsApp)
                }
              />
            )}
            {page === "设置" && (
              <SettingsPage
                settings={appSettingsState.appSettings}
                loadError={appSettingsState.loadError}
                onRetryLoad={appSettingsState.retryLoad}
                onRepair={() => void appSettingsState.repairSettings()}
                busy={busy}
                onPatch={appSettingsState.saveSettingsPatch}
                onRestart={() => void appSettingsState.restart()}
                updateCheck={updateCheck.updateCheck}
                updateChannel={updateCheck.updateChannel}
                updateChecking={updateCheck.checking}
                updateInstalling={updateCheck.installing}
                updateProgress={updateCheck.downloadProgress}
                updateCheckedAt={updateCheck.lastCheckedAt}
                updateRestartRequired={updateCheck.restartRequired}
                onCheckUpdate={() => void updateCheck.runUpdateCheck()}
                onInstallUpdate={() => void updateCheck.installAvailableUpdate()}
                onRestartInstalledUpdate={() => void updateCheck.restartInstalledUpdate()}
              />
            )}
            <div hidden={page !== "用量"}>
              <UsagePage active={page === "用量"} />
            </div>
            <section className="asb-panel" hidden={page !== "会话"} aria-label="会话管理">
              <div className="asb-panel-heading">
                <h2 className="asb-panel-title">会话管理</h2>
              </div>
              <SessionManager active={page === "会话"} />
            </section>
            {page === "日志" && (
              <LogsPage
                logLevel={appSettingsState.appSettings?.runtimeLogLevel ?? null}
                busy={busy}
                onLogLevelChange={(runtimeLogLevel) =>
                  appSettingsState.saveSettingsPatch({ runtimeLogLevel })
                }
              />
            )}
            {page === "备份" && (
              <BackupsPage
                records={snapshot.backups}
                busy={busy}
                lastSwitch={lastSwitchOverall}
                cloudBackup={cloudBackup}
                onRestore={operations.runRestore}
                onUndo={operations.requestUndo}
                onOpenDir={openBackupFolder}
              />
            )}
            {page === "发现" && (
              <DiscoveryPage
                discovery={discoveryState.discovery}
                busy={busy}
                onScan={() => void discoveryState.runDiscovery()}
                onImport={(app) => void discoveryState.runImport(app)}
              />
            )}
            {page === "发现" && (
              <CcImportSection
                scan={ccImport.ccScan}
                selected={ccImport.ccSelected}
                result={ccImport.ccResult}
                busy={busy}
                onSelect={(key, checked) =>
                  ccImport.setCcSelected((current) => ({ ...current, [key]: checked }))
                }
                onScan={() => void ccImport.runCcScan()}
                onImport={() => void ccImport.runCcImport()}
              />
            )}
          </main>
        </AppErrorBoundary>
      </AppShell>
      <OperationConfirmSheets preview={preview} operations={operations} providers={providers} />
      <Toaster />
    </>
  );
}
