import type { RefCallback } from "react";
import type {
  AppKind,
  ConfigFileStatus,
  LockStatus,
  ProviderDraft,
  ProviderProfile,
  UsageQuery,
  LocalizedMessage,
} from "../api/client";
import type { ProviderView } from "../app/navigation";
import type { ActivationCandidate } from "../app/useProviderSwitchFlow";
import type { ActiveProfileRef } from "../lib/current-provider-name";
import { useI18n } from "../i18n";
import { ProviderEditor } from "../components/ProviderEditor";
import { ProviderList } from "../components/ProviderList";
import { ProviderWorkspaceShell } from "../components/ProviderWorkspaceShell";
import { UsageQueryWorkspace } from "../components/UsageQueryWorkspace";
import type { ProviderEditorSession } from "../app/useProviders";

interface ProvidersPageProps {
  view: ProviderView;
  onViewChange: (view: ProviderView) => void;
  active: boolean;
  routeContentRef: RefCallback<HTMLElement>;
  /** Keeps the list mounted for one exit cycle while a tab switch replaces it. */
  leaving: boolean;
  profiles: ProviderProfile[];
  activeProfileId: string | null;
  statuses: ConfigFileStatus[] | null;
  /** Every stored profile across both clients; feeds the route cards'
   * active-profile lookups (name and website) above this page's list. */
  relayProfiles: readonly ActiveProfileRef[];
  locks: Partial<Record<AppKind, LockStatus>>;
  /** Model read from this client's user-level configuration file. */
  userConfigModel: string | null;
  /** Known conditions that can override the user-level configuration. */
  userConfigWarnings: LocalizedMessage[];
  /** The generic editor serves Claude only; Codex owns its specialized editor. */
  editorSession: Extract<ProviderEditorSession, { app: "claude" }> | null;
  busy: boolean;
  /** Persisted profile ids whose usage details are expanded. */
  expandedUsageIds: string[];
  /** The pending switch candidate; its row unfolds the write confirmation. */
  activationCandidate: ActivationCandidate | null;
  onSelectApp: (app: AppKind) => void;
  onNew: () => void;
  onImport: () => void;
  onCloseEditor: () => void;
  onSave: (draft: ProviderDraft) => Promise<void>;
  /** Replaces the editor session when the user picks Codex from the editor. */
  onSwitchClient: (app: AppKind) => void;
  onSaveUsageQuery: (
    profile: ProviderProfile,
    usageQuery: UsageQuery | null,
  ) => Promise<boolean>;
  onReorder: (orderedIds: string[]) => void;
  /** Persists the flipped usage-panel state for the profile. */
  onToggleUsage: (profile: ProviderProfile) => void;
  onActivate: (profile: ProviderProfile) => void;
  /** Confirms the pending switch candidate through the switch executor. */
  onConfirmSwitch: () => void;
  /** Discards the pending switch candidate without writing. */
  onCancelActivation: () => void;
  onEdit: (profile: ProviderProfile) => void;
  onRoutes: (profile: ProviderProfile) => void;
  onDelete: (profile: ProviderProfile) => void;
  /** Refreshes the provider snapshot after a management-dialog change. */
  onRefresh?: () => Promise<void>;
}

function ProviderEditView(props: ProvidersPageProps) {
  const { profiles, editorSession, busy } = props;
  if (!editorSession) return null;
  const profile = editorSession.record?.profile ?? null;
  return (
    <ProviderEditor
      key={profile?.id ?? `new-${editorSession.app}`}
      active={props.active}
      profile={profile}
      initialApp={editorSession.app}
      busy={busy}
      officialTakenApps={profiles
        .filter((profile) => profile.routeMode === "official")
        .map((profile) => profile.app)}
      onOpenOfficial={(app) => {
        const official = profiles.find(
          (profile) => profile.app === app && profile.routeMode === "official",
        );
        if (official) {
          props.onSelectApp(app);
          props.onEdit(official);
        }
      }}
      userConfigModel={props.userConfigModel}
      userConfigWarnings={props.userConfigWarnings}
      onSave={props.onSave}
      onSwitchClient={props.onSwitchClient}
      onCancel={props.onCloseEditor}
    />
  );
}

function ProviderListView({
  onConfigureUsage,
  ...props
}: ProvidersPageProps & {
  onConfigureUsage: (profile: ProviderProfile) => void;
}) {
  const { t } = useI18n();
  const { busy } = props;
  return (
    <ProviderWorkspaceShell
      ariaLabel={t("providers.workspace.aria")}
      app="claude"
      leaving={props.leaving}
      routeContentRef={props.routeContentRef}
      onSelectApp={props.onSelectApp}
      busy={busy}
      statuses={props.statuses}
      profiles={props.relayProfiles}
      locks={props.locks}
      onImport={props.onImport}
      onNew={props.onNew}
    >
      <ProviderList
        profiles={props.profiles.filter((profile) => profile.app === "claude")}
        activeProfileId={props.activeProfileId}
        userConfigModel={props.userConfigModel}
        userConfigWarnings={props.userConfigWarnings}
        busy={busy}
        expandedUsageIds={props.expandedUsageIds}
        activationCandidate={props.activationCandidate}
        onReorder={props.onReorder}
        onToggleUsage={props.onToggleUsage}
        onActivate={props.onActivate}
        onConfirmSwitch={props.onConfirmSwitch}
        onCancelActivation={props.onCancelActivation}
        onEdit={props.onEdit}
        onRoutes={props.onRoutes}
        onConfigureUsage={onConfigureUsage}
        onDelete={props.onDelete}
      />
    </ProviderWorkspaceShell>
  );
}

/** Editors retain their drafts while inactive; only the visible list mounts polling cards. */
export function ProvidersPage(props: ProvidersPageProps) {
  const usageProfile = props.view.kind === "usage" ? props.view.profile : null;
  if (usageProfile) {
    return (
      <div className="asb-editor-route" hidden={!props.active}>
        <UsageQueryWorkspace
          key={usageProfile.id}
          providerName={usageProfile.name}
          value={usageProfile.usageQuery ?? null}
          apiKey={usageProfile.apiKey}
          authentication={usageProfile.authentication}
          connection={usageProfile.connection}
          baseUrl={usageProfile.baseUrl}
          upstreamProtocol={usageProfile.upstreamProtocol}
          busy={props.busy}
          onSave={async (usageQuery) => {
            const saved = await props.onSaveUsageQuery(
              usageProfile,
              usageQuery,
            );
            if (saved) props.onViewChange({ kind: "list" });
            return saved;
          }}
          onClose={() => props.onViewChange({ kind: "list" })}
        />
      </div>
    );
  }
  if (props.editorSession !== null) {
    return (
      <div className="asb-editor-route" hidden={!props.active}>
        <ProviderEditView {...props} />
      </div>
    );
  }
  return props.active || props.leaving ? (
    <ProviderListView
      {...props}
      onConfigureUsage={(profile) =>
        props.onViewChange({ kind: "usage", profile })
      }
    />
  ) : null;
}
