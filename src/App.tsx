import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { AppShell } from "./app/AppShell";
import { AppWorkspace } from "./app/AppWorkspace";
import { useDevtoolsShortcut, useKeyboardFocusMarker, useScrollActivityMarker } from "./app/global-effects";
import { OperationConfirmSheets } from "./app/OperationConfirmSheets";
import { useSwitchboardModel } from "./app/useSwitchboardModel";

import { ApplicationPreparation as PreparationScreen } from "./components/ApplicationPreparation";
import { useApplicationPreparation } from "./app/useApplicationPreparation";

export default function App() {
  const preparation = useApplicationPreparation();
  useDevtoolsShortcut();
  useKeyboardFocusMarker();
  useScrollActivityMarker();
  if (!preparation.activated) return <PreparationScreen snapshot={preparation.snapshot} error={preparation.error}
    onRecheck={() => void preparation.recheck()} />;
  return <PreparedApp preparation={preparation} />;
}

function PreparedApp({ preparation }: { preparation: ReturnType<typeof useApplicationPreparation> }) {
  const model = useSwitchboardModel(preparation.snapshot!, preparation.completedRevision);
  const { page, setPage, error, busy, providers, operations, appSettingsState, update } = model;
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
  const suspended = preparation.snapshot?.status === "running" || preparation.snapshot?.status === "blocked" || !!preparation.error;
  return (
    <>
      <div className="asb-prepared-app" inert={suspended}>
      <AppShell page={page} onPageChange={setPage} error={error} busy={busy}
        sidebar={model.sidebar}
        preparation={preparation.snapshot!} settingsError={appSettingsState.loadFailure}
        pin={appSettingsState.pin} update={update} onOpenUpdate={() => model.openSettings("about")}>
        <AppErrorBoundary><AppWorkspace model={model} /></AppErrorBoundary>
      </AppShell>
      <OperationConfirmSheets operations={operations} providers={providers} busy={busy} />
      </div>
      {suspended &&
        <PreparationScreen snapshot={preparation.snapshot} error={preparation.error} overlay
          onRecheck={() => void preparation.recheck()} />}
    </>
  );
}
