import type {
  AppKind,
  FilePreview,
  ProviderDraft,
  ProviderProfile,
  UsageQuery,
} from "../api/client";
import { useState } from "react";
import { Button } from "../components/Button";
import { ClientLogo } from "../components/ClientLogo";
import { PlusIcon } from "../components/icons";
import { PreviewInspector } from "../components/PreviewInspector";
import { ProviderEditor } from "../components/ProviderEditor";
import { ProviderList } from "../components/ProviderList";
import { Tooltip } from "../components/Tooltip";
import { UsageQueryWorkspace } from "../components/UsageQueryWorkspace";
import type { EditorMode } from "../app/useProviders";
import { clientName } from "../lib/client-name";

interface ProvidersPageProps {
  profiles: ProviderProfile[];
  appFilter: AppKind;
  activeProfileId: string | null;
  /** Model read from this client's user-level configuration file. */
  userConfigModel: string | null;
  /** Known conditions that can override the user-level configuration. */
  userConfigWarnings: string[];
  selectedId: string | null;
  selectedProfile: ProviderProfile | null;
  editorMode: EditorMode;
  preview: { profileId: string; file: FilePreview } | null;
  busy: boolean;
  /** Persisted profile ids whose usage panel is collapsed. */
  collapsedUsageIds: string[];
  onSelectApp: (app: AppKind) => void;
  onNew: () => void;
  onCloseEditor: () => void;
  onSave: (draft: ProviderDraft) => Promise<void>;
  onSaveUsageQuery: (profile: ProviderProfile, usageQuery: UsageQuery | null) => Promise<boolean>;
  onSelect: (profileId: string) => void;
  onReorder: (orderedIds: string[]) => void;
  /** Persists the flipped usage-panel state for the profile. */
  onToggleUsage: (profile: ProviderProfile) => void;
  onActivate: (profile: ProviderProfile) => void;
  onTogglePreview: (profile: ProviderProfile) => void;
  onEdit: (profile: ProviderProfile) => void;
  onDelete: (profile: ProviderProfile) => void;
  onRequestSwitch: () => void;
  onCancelPreview: () => void;
}

/** Provider workspace: client tabs, the list with its inline preview, and
 * the dedicated editor view. */
export function ProvidersPage({
  profiles,
  appFilter,
  activeProfileId,
  userConfigModel,
  userConfigWarnings,
  selectedId,
  selectedProfile,
  editorMode,
  preview,
  busy,
  collapsedUsageIds,
  onSelectApp,
  onNew,
  onCloseEditor,
  onSave,
  onSaveUsageQuery,
  onSelect,
  onReorder,
  onActivate,
  onTogglePreview,
  onEdit,
  onDelete,
  onToggleUsage,
  onRequestSwitch,
  onCancelPreview,
}: ProvidersPageProps) {
  const [usageProfile, setUsageProfile] = useState<ProviderProfile | null>(null);
  /** Clients that already own their single official profile. */
  const officialTakenApps = profiles
    .filter((profile) => profile.routeMode === "official")
    .map((profile) => profile.app);

  if (usageProfile) {
    return (
      <div className="asb-edit-view">
        <section className="asb-panel asb-edit-panel">
          <UsageQueryWorkspace
            key={usageProfile.id}
            providerName={usageProfile.name}
            value={usageProfile.usageQuery ?? null}
            apiKey={usageProfile.apiKey}
            baseUrl={usageProfile.baseUrl}
            busy={busy}
            onSave={async (usageQuery) => {
              const saved = await onSaveUsageQuery(usageProfile, usageQuery);
              if (saved) setUsageProfile(null);
              return saved;
            }}
            onClose={() => setUsageProfile(null)}
          />
        </section>
      </div>
    );
  }

  if (editorMode !== null) {
    return (
      <div className="asb-edit-view">
        <div className="asb-edit-header">
          <Button
            variant="back"
            aria-label="返回供应商列表"
            disabled={busy}
            onClick={onCloseEditor}
          >
            ←
          </Button>
          <h2 className="asb-panel-title">
            {editorMode === "new" ? "新建供应商" : "编辑供应商"}
          </h2>
        </div>
        <section className="asb-panel asb-edit-panel">
          <ProviderEditor
            profile={editorMode === "edit" ? selectedProfile : null}
            initialApp={appFilter}
            busy={busy}
            officialTakenApps={officialTakenApps}
            onOpenOfficial={(app) => {
              const officialProfile = profiles.find(
                (profile) => profile.app === app && profile.routeMode === "official",
              );
              if (!officialProfile) return;
              onSelectApp(app);
              onEdit(officialProfile);
            }}
            userConfigModel={userConfigModel}
            userConfigWarnings={userConfigWarnings}
            onSave={onSave}
            onCancel={onCloseEditor}
          />
        </section>
      </div>
    );
  }

  const visibleProfiles = profiles.filter((profile) => profile.app === appFilter);
  return (
    <section className="asb-panel" aria-label="供应商工作区">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">供应商</h2>
      </div>
      <div className="asb-tabs-bar">
        <div className="asb-tabs" role="tablist" aria-label="客户端">
          {(["codex", "claude"] as const).map((app) => (
            <Tooltip key={app} label={clientName(app)} side="bottom">
              <button
                type="button"
                role="tab"
                aria-selected={appFilter === app}
                aria-label={clientName(app)}
                className={`asb-tab${appFilter === app ? " is-on" : ""}`}
                onClick={() => onSelectApp(app)}
              >
                <ClientLogo app={app} className="asb-tab-logo" />
              </button>
            </Tooltip>
          ))}
        </div>
        <Button
          variant="plus"
          disabled={busy}
          onClick={onNew}
        >
          <PlusIcon />
          新建供应商
        </Button>
      </div>
      <ProviderList
        profiles={visibleProfiles}
        activeProfileId={activeProfileId}
        userConfigModel={userConfigModel}
        selectedId={selectedId}
        openPreviewId={preview?.profileId ?? null}
        collapsedUsageIds={collapsedUsageIds}
        onSelect={onSelect}
        onReorder={onReorder}
        onToggleUsage={onToggleUsage}
        onActivate={onActivate}
        onPreview={onTogglePreview}
        onEdit={onEdit}
        onConfigureUsage={setUsageProfile}
        onDelete={onDelete}
        renderPreview={() =>
          preview && (
            <section className="asb-preview-inline" aria-label="变更预览">
              <div className="asb-panel-heading">
                <h3 className="asb-panel-title">变更预览</h3>
                <div className="asb-panel-actions">
                  <Button
                    variant="secondary"
                    disabled={busy}
                    onClick={onCancelPreview}
                  >
                    取消
                  </Button>
                  <Button
                    variant="primary"
                    disabled={busy}
                    onClick={onRequestSwitch}
                  >
                    确认切换
                  </Button>
                </div>
              </div>
              <PreviewInspector
                filePreview={preview.file}
                userConfigModel={userConfigModel}
                userConfigWarnings={userConfigWarnings}
              />
            </section>
          )
        }
      />
    </section>
  );
}
