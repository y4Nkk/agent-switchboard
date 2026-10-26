import { useMemo, useRef, useState, type ReactNode } from "react";
import type { CodexProviderRecord, ProviderProfile, ProviderRecord, ProviderRequestTarget } from "../api/client";
import { queryCodexOfficialQuota, readCodexOfficialQuota } from "../api/client";
import { useI18n } from "../i18n";
import { CodexOfficialQuotaPanel } from "./CodexOfficialQuotaPanel";
import { OfficialLoginPanel } from "./OfficialLoginPanel";
import { ProviderRowShell } from "./ProviderWorkspaceShell";
import { ProviderTestPanel } from "./ProviderTestPanel";
import { ProviderUsagePanel } from "./ProviderUsagePanel";
import { Collapsible } from "./Collapsible";
import { useProviderUsage, type ProviderUsage } from "./use-provider-usage";
import { ProviderUsageSummary } from "./ProviderUsageSummary";
import { ProviderActivateButton, ProviderEndpoint, ProviderLoginButton, ProviderRowActions } from "./ProviderRowControls";
import { OfficialQuotaSummary } from "./OfficialQuotaSummary";
import { useCachedQuery } from "./use-cached-query";

interface RowProps {
  onRoutes?: () => void;
  active: boolean;
  confirmationOpen: boolean;
  onActivate: () => void;
  onEdit: () => void;
  onDelete: () => void;
  children?: ReactNode;
}

interface ThirdPartyRowProps extends RowProps {
  record: CodexProviderRecord;
  /** Persisted disclosure state of the configured usage panel. */
  usageOpen: boolean;
  onToggleUsage: () => void;
  onConfigureUsage: () => void;
  /** Present only on configured rows, which subscribe to cached usage. */
  usage?: ProviderUsage;
}

/** Third-party Codex profiles share usage and action components with Claude. */
export function CodexProviderRow({ record, usageOpen, onToggleUsage, onConfigureUsage, usage, ...props }: ThirdPartyRowProps) {
  const { id, name } = record.profile;
  const [testOpen, setTestOpen] = useState(false);
  const testTriggerRef = useRef<HTMLButtonElement>(null);
  const target = useMemo<ProviderRequestTarget>(() => ({ kind: "saved", profileId: id }), [id]);
  const testId = `provider-test-${id}`;
  const usageId = `provider-usage-${id}`;
  const hasUsageQuery = Boolean(record.usageQuery);
  return <ProviderRowShell id={id} name={name} active={props.active}
    confirmationOpen={props.confirmationOpen} sortable model={record.profile.defaultModel}
    endpoint={<ProviderEndpoint url={record.websiteUrl || record.profile.endpoint} link={Boolean(record.websiteUrl)} />}
    summary={usage ? <ProviderUsageSummary name={name} usage={usage} /> : undefined}
    primaryAction={!props.active ? <ProviderActivateButton name={name} onActivate={props.onActivate} /> : undefined}
    actions={<ProviderRowActions name={name} onRoutes={props.onRoutes} onEdit={props.onEdit} onDelete={props.onDelete}
      test={{ id: testId, open: testOpen, trigger: testTriggerRef, onToggle: () => setTestOpen((open) => !open) }}
      usage={{ id: usageId, configured: hasUsageQuery, open: usageOpen,
        onOpen: () => { if (hasUsageQuery) onToggleUsage(); else onConfigureUsage(); } }} />}>
    <Collapsible open={testOpen}>
      <ProviderTestPanel id={testId} name={name} url={record.profile.endpoint} target={target}
        onClose={() => { setTestOpen(false); testTriggerRef.current?.focus(); }} />
    </Collapsible>
    {usage && <Collapsible open={usageOpen}>
      <ProviderUsagePanel id={usageId} name={name} usage={usage} onConfigure={onConfigureUsage} />
    </Collapsible>}
    {props.children}
  </ProviderRowShell>;
}

/** Both clients subscribe to the same backend-owned usage cache. */
export function ConfiguredCodexProviderRow(props: ThirdPartyRowProps) {
  const usage = useProviderUsage({ app: "codex", id: props.record.profile.id, usageQuery: props.record.usageQuery });
  return <CodexProviderRow {...props} usage={usage} />;
}

interface OfficialRowProps extends RowProps {
  record: ProviderRecord;
  quotaOpen: boolean;
  onSaveQuotaInterval: (profile: ProviderProfile, minutes: number) => Promise<boolean>;
  onToggleQuota: () => void;
  onReloginFinished: () => void;
}

/** Official login participates in the Codex provider order while retaining its own login and quota state. */
export function CodexOfficialRow(props: OfficialRowProps) {
  const { t } = useI18n();
  const { profile } = props.record;
  const [reloginOpen, setReloginOpen] = useState(false);
  const quota = useCachedQuery(profile.id, String(profile.officialQuotaRefreshIntervalMinutes ?? 0),
    readCodexOfficialQuota, queryCodexOfficialQuota);
  return (
    <ProviderRowShell id={profile.id} name={profile.name} active={props.active}
      confirmationOpen={props.confirmationOpen} sortable model={profile.model ?? t("codex.rows.defaultModel")}
      endpoint={<span>{t("codex.identity.official")}</span>}
      summary={<OfficialQuotaSummary name={profile.name} quota={quota} />}
      primaryAction={!props.active ? <ProviderActivateButton name={profile.name} onActivate={props.onActivate} /> : undefined}
      secondaryAction={<ProviderLoginButton name={profile.name} open={reloginOpen} controlsId={`codex-official-relogin-${profile.id}`}
        onToggle={() => setReloginOpen((open) => !open)} />}
      actions={<ProviderRowActions name={profile.name} onRoutes={props.onRoutes} onEdit={props.onEdit} onDelete={props.onDelete}
        usage={{ id: `codex-official-quota-${profile.id}`, configured: true, open: props.quotaOpen, onOpen: props.onToggleQuota }} />}>
      <Collapsible open={reloginOpen} id={`codex-official-relogin-${profile.id}`}>
        <OfficialLoginPanel app="codex" onFinished={(completed) => {
          if (completed) {
            quota.run();
            props.onReloginFinished();
          }
        }} />
      </Collapsible>
      <Collapsible open={props.quotaOpen} id={`codex-official-quota-${profile.id}`}>
        <CodexOfficialQuotaPanel quota={quota} profileName={profile.name}
          refreshIntervalMinutes={profile.officialQuotaRefreshIntervalMinutes ?? 0}
          onSaveInterval={(minutes) => props.onSaveQuotaInterval(profile, minutes)} />
      </Collapsible>
      {props.children}
    </ProviderRowShell>
  );
}
