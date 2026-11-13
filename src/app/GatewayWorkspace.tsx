import { GatewayPage } from "../pages/GatewayPage";
import { ModelRoutingWorkspace } from "../components/model-routing/ModelRoutingWorkspace";
import type { SwitchboardModel } from "./useSwitchboardModel";

/** Runtime state and saved plans share the page; editing belongs to providers. */
export function GatewayWorkspace({ model, active }: { model: SwitchboardModel; active: boolean }) {
  const { appFilter, busy, snapshot, providers } = model;
  return (
    <GatewayPage busy={busy} active={active} profiles={snapshot.relayProfiles}
      routing={(summary, runtimeBusy) => <ModelRoutingWorkspace runtimeSummary={summary}
        initialApp={appFilter} busy={busy || runtimeBusy}
        inventory={{ codex: snapshot.codexRecords, profiles: [...snapshot.records, ...snapshot.codexOfficialRecords],
          activeIds: { codex: model.activeProfileId("codex"), claude: model.activeProfileId("claude") } }}
        onEdit={(app, profileId, role) => providers.openRoutes(app, profileId, role, "gateway")}
        onProviders={(app) => { model.providers.selectApp(app); model.setPage("providers"); }} />} />
  );
}
