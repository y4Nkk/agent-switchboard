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
        pin={appSettingsState.pin}
        update={
          updateCheck.updateCheck?.updateAvailable === true
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
                selectedProfile={selectedProfile}
                canSwitch={preview !== null}
                busy={busy}
                relayHidden={editorMode !== null}
                onPreview={() => void switchPreview.runPreview()}
                onRequestSwitch={requestSwitch}
                onRefresh={() => void snapshot.refresh()}
                onRecoverLock={operations.setRecoverLockPending}
              />
            )}
            {page === "供应商" && (
              <ProvidersPage
                profiles={snapshot.profiles}
                appFilter={appFilter}
                activeProfileId={activeProfileId(appFilter)}
                selectedId={snapshot.selectedId}
                selectedProfile={selectedProfile}
                editorMode={editorMode}
                preview={preview}
                busy={busy}
                onSelectApp={providers.selectApp}
                onNew={() => setEditorMode("new")}
                onCloseEditor={() => setEditorMode(null)}
                onSave={providers.saveProfile}
                onSaveUsageQuery={providers.saveProfileUsageQuery}
                onSelect={switchPreview.selectProfile}
                onReorder={providers.dragReorderProfiles}
                onActivate={switchPreview.previewProfile}
                onTogglePreview={switchPreview.togglePreviewProfile}
                onEdit={providers.openEditor}
                onDelete={providers.setDeletePending}
                onRequestSwitch={requestSwitch}
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
                promptApp={promptDocuments.promptApp}
                promptDocument={promptDocuments.documents[promptDocuments.promptApp]}
                promptDraft={
                  promptDocuments.drafts[promptDocuments.promptApp] ??
                  promptDocuments.documents[promptDocuments.promptApp]?.content ??
                  ""
                }
                promptDirty={promptDocuments.isPromptDirty(promptDocuments.promptApp)}
                onSelectPromptApp={promptDocuments.setPromptApp}
                onPromptDraftChange={(content) =>
                  promptDocuments.setPromptDraft(promptDocuments.promptApp, content)
                }
                onSavePrompt={() => void promptDocuments.savePromptDocument(promptDocuments.promptApp)}
                onDiscardPrompt={() => promptDocuments.discardPromptDraft(promptDocuments.promptApp)}
                onReloadPrompt={() => promptDocuments.reloadPromptDocument(promptDocuments.promptApp)}
              />
            )}
            {page === "设置" && (
              <SettingsPage
                settings={appSettingsState.appSettings}
                loadError={appSettingsState.loadError}
                onRetryLoad={appSettingsState.retryLoad}
                busy={busy}
                onPatch={appSettingsState.saveSettingsPatch}
                onRestart={() => void appSettingsState.restart()}
                updateCheck={updateCheck.updateCheck}
                onCheckUpdate={() => void updateCheck.runUpdateCheck()}
              />
            )}
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
