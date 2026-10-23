import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { AppShell } from "./app/AppShell";
import { AppWorkspace } from "./app/AppWorkspace";
import { useDevtoolsShortcut, useKeyboardFocusMarker, useScrollActivityMarker } from "./app/global-effects";
import { OperationConfirmSheets } from "./app/OperationConfirmSheets";
import { useSwitchboardModel } from "./app/useSwitchboardModel";

/** The shell composes the domain model, workspaces and operation confirmations.
 * The whole tree waits for the settings snapshot (or its load failure) so the
 * saved language is known before the first label renders — no wrong-language
 * flash. The rail also restores before mounting the shell. Accepted snapshots
 * publish the language before the UI state. */
export default function App() {
  const model = useSwitchboardModel();
  const { page, setPage, error, busy, providers, operations, appSettingsState, updateCheck } = model;
  useDevtoolsShortcut();
  useKeyboardFocusMarker();
  useScrollActivityMarker();
  const settingsReady = appSettingsState.appSettings !== null || appSettingsState.loadError !== null;
  if (!settingsReady || !model.sidebar.ready) {
    return (
      <main className="asb-splash" aria-label="Agent Switchboard">
        <div className="asb-settings-skeleton" role="status">
          <div className="asb-skeleton" /><div className="asb-skeleton" /><div className="asb-skeleton" />
        </div>
      </main>
    );
  }
  return (
    <>
      <AppShell page={page} onPageChange={setPage} error={error} busy={busy}
        sidebar={model.sidebar}
        onRepairData={() => void model.dataRepair.run()}
        settingsError={appSettingsState.loadError} onRepairSettings={() => void appSettingsState.repairSettings()}
        pin={appSettingsState.pin} update={{
          checking: updateCheck.checking,
          latestVersion: updateCheck.updateCheck?.latestVersion ?? null,
          // Store installs are updated by the Store; their entry only leads to
          // the settings section that says so, mirroring the settings page.
          onCheck: updateCheck.updateChannel === "microsoftStore"
            ? () => model.openSettings("about")
            : updateCheck.runUpdateCheck,
          onOpen: () => model.openSettings("about"),
        }}>
        <AppErrorBoundary><AppWorkspace model={model} /></AppErrorBoundary>
      </AppShell>
      <OperationConfirmSheets operations={operations} providers={providers} busy={busy} />
    </>
  );
}
