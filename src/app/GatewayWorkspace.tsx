import { GatewayPage } from "../pages/GatewayPage";
import { ModelRoutingWorkspace } from "../components/model-routing/ModelRoutingWorkspace";
import type { SwitchboardModel } from "./useSwitchboardModel";

/** Top-level "model routing & gateway" page: the routing workspace embeds the
 * runtime summary, while topology, telemetry and the port sheet live below. */
export function GatewayWorkspace({ model, active }: { model: SwitchboardModel; active: boolean }) {
  const { appFilter, busy, modelRoutes, setBusy, snapshot } = model;
  return (
    <GatewayPage busy={busy} active={active} profiles={snapshot.relayProfiles}
      routing={(summary, refreshRuntime, runtimeBusy) => <ModelRoutingWorkspace runtimeSummary={summary}
        active={active} initialApp={appFilter} request={modelRoutes.request} busy={busy || runtimeBusy}
        onBusy={setBusy}
        inventory={{ codex: snapshot.codexRecords, profiles: [...snapshot.records, ...snapshot.codexOfficialRecords],
          activeIds: { codex: model.activeProfileId("codex"), claude: model.activeProfileId("claude") } }}
        onApplied={async () => { model.providerSwitch.invalidateCandidates(); await Promise.all([snapshot.refresh(), refreshRuntime()]); }}
        onProviders={(app) => { model.providers.selectApp(app); model.setPage("providers"); }} />} />
  );
}
