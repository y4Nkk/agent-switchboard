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
    appSettingsState,
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
                onOpenSessions={() => setPage("会话")}
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
                toggles={commonSettings.toggles[commonSettings.settingsApp]}
                choices={commonSettings.choices[commonSettings.settingsApp]}
                commonPreview={commonSettings.commonPreview[commonSettings.settingsApp]}
                busy={busy}
                onApplyLine={(app, key, value) =>
                  void commonSettings.applyCommonLine(app, key, value)
                }
              />
            )}
            {page === "设置" && (
              <SettingsPage
                settings={appSettingsState.appSettings}
                busy={busy}
                onPatch={appSettingsState.saveSettingsPatch}
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
            {page === "备份" && (
              <BackupsPage
                records={snapshot.backups}
                busy={busy}
                lastSwitch={lastSwitchOverall}
                onRestore={operations.runRestore}
                onUndo={operations.setUndoPending}
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
