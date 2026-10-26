import { toastMessage } from "../components/use-toast";
import { useCallback, useEffect, useState, type RefCallback } from "react";
import {
  executeSwitch, reorderCodexProfiles,
  type AppKind, type CodexProviderDraft, type CodexProviderRecord, type CommandError,
  type ConfigFileStatus, type LockStatus, type ProviderDraft, type ProviderProfile,
  type ProviderRecord,
  type UsageQuery,
  type LocalizedMessage,
} from "../api/client";
import type { CodexEditorSource } from "../app/useProviders";
import { useProviderSwitchFlow } from "../app/useProviderSwitchFlow";
import { notifyWriteOutcome } from "../app/notifications";
import type { ActiveProfileRef } from "../lib/current-provider-name";
import { useI18n } from "../i18n";
import { CodexOfficialRow, CodexProviderRow, ConfiguredCodexProviderRow } from "../components/CodexProviderRows";
import { CodexProviderEditor } from "../components/codex-provider-editor/CodexProviderEditor";
import { ProviderWorkspaceShell, SortableProviderRows } from "../components/ProviderWorkspaceShell";
import { SwitchConfirmationSection } from "../components/SwitchConfirmationSection";
import { UsageQueryWorkspace } from "../components/UsageQueryWorkspace";

interface Props {
  active: boolean;
  routeContentRef: RefCallback<HTMLElement>;
  /** Keeps the list mounted for one exit cycle while a tab switch replaces it. */
  leaving: boolean;
  records: CodexProviderRecord[];
  /** The stored Codex official-login record, or null when the client has none.
   * Official login is not a third-party profile, so it never appears in
   * `records`; it is one more row the user may create from this page. */
  officialRecord: ProviderRecord | null;
  activeProfileId: string | null;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onError: (error: CommandError) => void;
  onRefresh: () => Promise<void>;
  onSelectApp: (app: AppKind) => void;
  onImport: () => void;
  /** Routes the destructive delete through the shared confirmation sheet. */
  onDelete: (record: CodexProviderRecord) => void;
  /** Official-login removal goes through the same sheet as a Claude profile. */
  onDeleteOfficial: (record: ProviderRecord) => void;
  /** Codex-only editor session; the shared session state keeps drafts alive
   * across pages exactly like the Claude editor. */
  editorSession: { source: CodexEditorSource | null } | null;
  onNew: () => void;
  onEdit: (record: CodexProviderRecord) => void;
  onRoutes: (profileId: string) => void;
  /** Opens the official-login editor arm; null record creates the entry. */
  onEditOfficial: (record: ProviderRecord | null) => void;
  onCloseEditor: () => void;
  onSave: (draft: CodexProviderDraft) => void;
  onSaveOfficial: (draft: ProviderDraft) => void;
  onSwitchAccessMode: (official: boolean, existing: ProviderRecord | null) => void;
  onSwitchClient: (app: AppKind) => void;
  /** Persists the official record's subscription-quota refresh interval. */
  onSaveOfficialQuotaInterval: (profile: ProviderProfile, minutes: number) => Promise<boolean>;
  /** Persisted profile ids whose usage details are expanded; all other
   * configured panels stay collapsed. */
  expandedUsageIds: string[];
  /** Persists the flipped usage-panel state for a Codex profile. */
  onToggleUsage: (profileId: string) => void;
  /** Saves one Codex profile's usage query through the strict store. */
  onSaveUsageQuery: (record: CodexProviderRecord, usageQuery: UsageQuery | null) => Promise<boolean>;
  /** Why third-party switching is blocked right now, or null when the
   * official login is ready; owned by the config snapshot refresh. */
  loginBlocker: string | null;
  statuses: ConfigFileStatus[] | null;
  /** Every stored profile across both clients; feeds the route cards'
   * active-profile lookups (name and website) above this page's list. */
  relayProfiles: readonly ActiveProfileRef[];
  locks: Partial<Record<AppKind, LockStatus>>;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
}

function useCodexProvidersState(props: Props) {
  const [quotaOpen, setQuotaOpen] = useState(false);
  /** The Codex provider whose usage query the full-page workspace edits. */
  const [usageRecord, setUsageRecord] = useState<CodexProviderRecord | null>(null);
  const providerSwitch = useProviderSwitchFlow({ busy: props.busy, onError: props.onError });
  const { activationCandidate, clearCandidates, requestActivation } = providerSwitch;
  const run = useCallback(async (action: () => Promise<void>) => {
    if (props.busy) return;
    props.onBusy(true);
    try { await action(); }
    catch (caught) { props.onError(caught as CommandError); }
    finally { props.onBusy(false); }
  }, [props.busy, props.onBusy, props.onError]);

  useEffect(() => {
    if (!props.active || props.editorSession) clearCandidates();
  }, [props.active, props.editorSession, clearCandidates]);

  const confirmActivation = () => {
    const candidate = activationCandidate;
    if (!candidate) return;
    void run(async () => {
      const outcome = await executeSwitch(candidate.profileId, candidate.file.contentHash,
        candidate.file.renderedHash, true, candidate.file);
      clearCandidates();
      notifyWriteOutcome(toastMessage("codex.page.switchedToast"), "codex", outcome.warnings);
      await props.onRefresh();
    });
  };
  const reorder = (orderedIds: string[], expectedFileHashes: Record<string, string>) => void run(async () => {
    await reorderCodexProfiles(orderedIds, expectedFileHashes);
    await props.onRefresh();
  });
  return {
    activationCandidate,
    clearCandidates,
    quotaOpen,
    setQuotaOpen,
    usageRecord,
    setUsageRecord,
    requestActivation,
    run,
    confirmActivation,
    reorder,
  };
}

type PageState = ReturnType<typeof useCodexProvidersState>;

function CodexProvidersList({ props, state }: { props: Props; state: PageState }) {
  const { t } = useI18n();
  const { activationCandidate } = state;
  const official = props.officialRecord;
  const rows = [
    ...(official ? [{ kind: "official" as const, record: official }] : []),
    ...props.records.map((record) => ({ kind: "provider" as const, record })),
  ].sort((left, right) => left.record.position - right.record.position
    || left.record.profile.id.localeCompare(right.record.profile.id));
  const confirmationSection = activationCandidate && (
    <SwitchConfirmationSection filePreview={activationCandidate.file}
      busy={props.busy} userConfigModel={props.userConfigModel}
      userConfigWarnings={props.userConfigWarnings}
      onConfirm={state.confirmActivation} onCancel={state.clearCandidates} />
  );
  const rowProps = (profile: { id: string }) => ({
    active: props.activeProfileId === profile.id,
    confirmationOpen: activationCandidate?.profileId === profile.id,
    onActivate: () => void state.requestActivation(profile),
  });
  return (
    <>
      <SortableProviderRows
        ids={rows.map((row) => row.record.profile.id)}
        onReorder={(orderedIds) => state.reorder(orderedIds,
          Object.fromEntries(rows.map((row) => [row.record.profile.id, row.record.fileHash])))}
        ariaLabel={t("codex.page.listAria")}
        emptyLabel={t("codex.page.empty")}
      >
        {rows.map((row) => {
          if (row.kind === "official") {
            return (
              <CodexOfficialRow key={row.record.profile.id} record={row.record} {...rowProps(row.record.profile)} quotaOpen={state.quotaOpen}
                onEdit={() => { state.clearCandidates(); props.onEditOfficial(row.record); }}
                onRoutes={() => props.onRoutes(row.record.profile.id)}
                onDelete={() => props.onDeleteOfficial(row.record)}
                onSaveQuotaInterval={props.onSaveOfficialQuotaInterval}
                onToggleQuota={() => state.setQuotaOpen((open) => !open)}
                onReloginFinished={() => void props.onRefresh()}>
                {activationCandidate?.profileId === row.record.profile.id && confirmationSection}
              </CodexOfficialRow>
            );
          }
          const Row = row.record.usageQuery ? ConfiguredCodexProviderRow : CodexProviderRow;
          return (
            <Row key={row.record.profile.id} record={row.record} {...rowProps(row.record.profile)}
              usageOpen={props.expandedUsageIds.includes(row.record.profile.id)}
              onToggleUsage={() => props.onToggleUsage(row.record.profile.id)}
              onConfigureUsage={() => state.setUsageRecord(row.record)}
              onEdit={() => { state.clearCandidates(); props.onEdit(row.record); }}
              onRoutes={() => props.onRoutes(row.record.profile.id)}
              onDelete={() => props.onDelete(row.record)}>
              {activationCandidate?.profileId === row.record.profile.id && confirmationSection}
            </Row>
          );
        })}
      </SortableProviderRows>
    </>
  );
}

export function CodexProvidersPage(props: Props) {
  const { t } = useI18n();
  const state = useCodexProvidersState(props);
  if (props.editorSession) return (
    <div className="asb-editor-route" hidden={!props.active}>
      <CodexProviderEditor active={props.active} source={props.editorSession.source} busy={props.busy}
        userConfigModel={props.userConfigModel} userConfigWarnings={props.userConfigWarnings}
        onSave={props.onSave} onSaveOfficial={props.onSaveOfficial}
        onSwitchAccessMode={props.onSwitchAccessMode} onCancel={props.onCloseEditor}
        onSwitchClient={props.onSwitchClient} />
    </div>
  );
  const usageRecord = state.usageRecord;
  if (usageRecord) return (
    <div className="asb-editor-route" hidden={!props.active}>
      <UsageQueryWorkspace
        key={usageRecord.profile.id}
        providerName={usageRecord.profile.name}
        value={usageRecord.usageQuery ?? null}
        apiKey={usageRecord.profile.apiKey}
        authentication={usageRecord.profile.authentication ?? null}
        connection={usageRecord.profile.connection ?? null}
        baseUrl={usageRecord.profile.endpoint}
        upstreamProtocol={usageRecord.profile.upstream}
        busy={props.busy}
        onSave={async (usageQuery) => {
          const saved = await props.onSaveUsageQuery(usageRecord, usageQuery);
          if (saved) state.setUsageRecord(null);
          return saved;
        }}
        onClose={() => state.setUsageRecord(null)}
      />
    </div>
  );
  if (!props.active && !props.leaving) return null;
  return (
      <ProviderWorkspaceShell ariaLabel={t("codex.page.providersAria")} app="codex"
      leaving={props.leaving} onSelectApp={props.onSelectApp} routeContentRef={props.routeContentRef}
      busy={props.busy} statuses={props.statuses} profiles={props.relayProfiles} locks={props.locks}
      onImport={() => { state.clearCandidates(); props.onImport(); }}
      onNew={() => { state.clearCandidates(); props.onNew(); }}>
      <CodexProvidersList props={props} state={state} />
    </ProviderWorkspaceShell>
  );
}
