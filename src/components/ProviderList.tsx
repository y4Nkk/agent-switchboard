import type { ActivationCandidate } from "../app/useProviderSwitchFlow";
import type { ProviderProfile, ProviderRequestTarget, LocalizedMessage } from "../api/client";
import { useMemo, useRef, useState, type ReactNode } from "react";
import { useI18n } from "../i18n";
import { OfficialLoginPanel } from "./OfficialLoginPanel";
import { ProviderRowShell, SortableProviderRows } from "./ProviderWorkspaceShell";
import { ProviderUsagePanel } from "./ProviderUsagePanel";
import { Collapsible } from "./Collapsible";
import { useProviderUsage, type ProviderUsage } from "./use-provider-usage";
import { ProviderUsageSummary } from "./ProviderUsageSummary";
import { SwitchConfirmationSection } from "./SwitchConfirmationSection";
import { ProviderActivateButton, ProviderEndpoint, ProviderLoginButton, ProviderRowActions } from "./ProviderRowControls";
import { ProviderTestPanel } from "./ProviderTestPanel";

interface Props {
  profiles: ProviderProfile[];
  /** Profile id the live file actually matches, when the app can tell. */
  activeProfileId: string | null;
  /** Model read from the displayed client's user-level configuration file. */
  userConfigModel: string | null;
  /** Known conditions that can override the user-level configuration. */
  userConfigWarnings: LocalizedMessage[];
  busy: boolean;
  /** Persisted profile ids whose usage details are expanded; all other
   * configured panels stay collapsed. */
  expandedUsageIds?: string[];
  /** The pending switch candidate; its row unfolds the confirmation. */
  activationCandidate: ActivationCandidate | null;
  /** Persists a new display order for the visible client's profiles. */
  onReorder?: (orderedIds: string[]) => void;
  /** Persists the flipped usage-panel state for the profile. */
  onToggleUsage?: (profile: ProviderProfile) => void;
  /** Requests a fresh candidate and the explicit write confirmation. */
  onActivate?: (profile: ProviderProfile) => void;
  /** Confirms the pending candidate through the switch executor. */
  onConfirmSwitch: () => void;
  /** Discards the pending candidate without writing. */
  onCancelActivation: () => void;
  onEdit?: (profile: ProviderProfile) => void;
  onRoutes?: (profile: ProviderProfile) => void;
  /** Opens the dedicated usage-query workspace for this profile. */
  onConfigureUsage?: (profile: ProviderProfile) => void;
  onDelete?: (profile: ProviderProfile) => void;
}

interface RowProps {
  onRoutes?: (profile: ProviderProfile) => void;
  profile: ProviderProfile;
  active: boolean;
  userConfigModel: string | null;
  usageOpen: boolean;
  sortable: boolean;
  /** The pending switch confirmation, already matched to this row. */
  confirmation?: ReactNode;
  onToggleUsage: (profile: ProviderProfile) => void;
  onActivate?: (profile: ProviderProfile) => void;
  onEdit?: (profile: ProviderProfile) => void;
  onConfigureUsage?: (profile: ProviderProfile) => void;
  onDelete?: (profile: ProviderProfile) => void;
}

function ConfiguredProviderRow(props: RowProps) {
  const usage = useProviderUsage(props.profile);
  return <ProviderRow {...props} usage={usage} />;
}

/** The model rail stays fixed while complete usage facts live below it. */
function ProviderRow(props: RowProps & { usage?: ProviderUsage }) {
  const { t } = useI18n();
  const { profile, active, userConfigModel, usageOpen, sortable, confirmation, usage } = props;
  const [reloginOpen, setReloginOpen] = useState(false);
  const [testOpen, setTestOpen] = useState(false);
  const testTriggerRef = useRef<HTMLButtonElement>(null);
  const target = useMemo<ProviderRequestTarget>(() => ({ kind: "saved", profileId: profile.id }),
    [profile.id, profile.baseUrl, profile.apiKey, profile.upstreamProtocol, profile.model, profile.responsesOptions?.requestMode]);
  const official = profile.routeMode === "official";
  const modelText = (active ? userConfigModel : profile.model) ?? t("providers.label.defaultModel");
  const modelTitle = active ? t("providers.row.modelFromConfig", { model: modelText }) : t("providers.row.modelFromProfile", { model: modelText });
  const hasUsageQuery = Boolean(profile.usageQuery);
  const testId = `provider-test-${profile.id}`;
  const usageId = `provider-usage-${profile.id}`;
  const hasActions = Boolean(props.onRoutes || props.onEdit || props.onDelete || (!official && (profile.baseUrl || hasUsageQuery || props.onConfigureUsage)));
  return <ProviderRowShell id={profile.id} name={profile.name} active={active}
    confirmationOpen={confirmation !== undefined} sortable={sortable}
    model={<span title={modelTitle}>{modelText}</span>}
    endpoint={profile.websiteUrl ? <ProviderEndpoint url={profile.websiteUrl} link /> : official ? <span>{t("providers.label.officialLogin")}</span> : undefined}
    summary={usage ? <ProviderUsageSummary name={profile.name} usage={usage} /> : undefined}
    primaryAction={!active && props.onActivate ? <ProviderActivateButton name={profile.name} onActivate={() => props.onActivate?.(profile)} /> : undefined}
    secondaryAction={official ? <ProviderLoginButton name={profile.name} open={reloginOpen}
      controlsId={`provider-relogin-${profile.id}`} onToggle={() => setReloginOpen((open) => !open)} /> : undefined}
    actions={hasActions ? <ProviderRowActions name={profile.name}
      onRoutes={props.onRoutes ? () => props.onRoutes?.(profile) : undefined}
      onEdit={props.onEdit ? () => props.onEdit?.(profile) : undefined}
      onDelete={props.onDelete ? () => props.onDelete?.(profile) : undefined}
      test={!official && profile.baseUrl ? { id: testId, open: testOpen, trigger: testTriggerRef, onToggle: () => setTestOpen((open) => !open) } : undefined}
      usage={!official && (hasUsageQuery || props.onConfigureUsage) ? {
        id: usageId, configured: hasUsageQuery, open: usageOpen,
        onOpen: () => { if (hasUsageQuery) props.onToggleUsage(profile); else props.onConfigureUsage?.(profile); },
      } : undefined} /> : undefined}>
    {!official && profile.baseUrl && <Collapsible open={testOpen}>
      <ProviderTestPanel id={testId} name={profile.name} url={profile.baseUrl} target={target}
        onClose={() => { setTestOpen(false); testTriggerRef.current?.focus(); }} />
    </Collapsible>}
    {usage && <Collapsible open={usageOpen}>
      <ProviderUsagePanel id={usageId} name={profile.name} usage={usage}
        onConfigure={props.onConfigureUsage ? () => props.onConfigureUsage?.(profile) : undefined} />
    </Collapsible>}
    {confirmation}
    <Collapsible open={reloginOpen} id={`provider-relogin-${profile.id}`}>
      <OfficialLoginPanel app={profile.app} />
    </Collapsible>
  </ProviderRowShell>;
}

/** Claude's list; Codex rows and official quotas belong to CodexProvidersPage. */
export function ProviderList({
  profiles,
  activeProfileId,
  userConfigModel,
  userConfigWarnings,
  busy,
  expandedUsageIds = [],
  activationCandidate,
  onReorder,
  onToggleUsage,
  onActivate,
  onConfirmSwitch,
  onCancelActivation,
  onEdit,
  onRoutes,
  onConfigureUsage,
  onDelete,
}: Props) {
  const ids = profiles.map((profile) => profile.id);
  return (
    <SortableProviderRows ids={ids} onReorder={onReorder}>
      {profiles.map((profile) => {
        const Row = profile.usageQuery ? ConfiguredProviderRow : ProviderRow;
        const confirming = activationCandidate?.profileId === profile.id;
        return <Row
          key={profile.id}
          profile={profile}
          active={profile.id === activeProfileId}
          userConfigModel={userConfigModel}
          usageOpen={Boolean(profile.usageQuery) && expandedUsageIds.includes(profile.id)}
          sortable={Boolean(onReorder)}
          confirmation={confirming && activationCandidate ? (
            <SwitchConfirmationSection filePreview={activationCandidate.file}
              busy={busy} userConfigModel={userConfigModel}
              userConfigWarnings={userConfigWarnings}
              onConfirm={onConfirmSwitch} onCancel={onCancelActivation} />
          ) : undefined}
          onToggleUsage={(toggled) => onToggleUsage?.(toggled)}
          onActivate={onActivate}
          onEdit={onEdit}
          onRoutes={onRoutes}
          onConfigureUsage={onConfigureUsage}
          onDelete={onDelete}
        />;
      })}
    </SortableProviderRows>
  );
}
