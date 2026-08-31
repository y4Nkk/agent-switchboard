import type {
  AppKind,
  FilePreview,
  ProviderDraft,
  ProviderProfile,
} from "../api/client";
import { ClientLogo } from "../components/ClientLogo";
import { PlusIcon } from "../components/icons";
import { PreviewInspector } from "../components/PreviewInspector";
import { ProviderEditor } from "../components/ProviderEditor";
import { ProviderList } from "../components/ProviderList";
import { Tooltip } from "../components/Tooltip";
import type { EditorMode } from "../app/useProviders";
import { clientName } from "../lib/client-name";

interface ProvidersPageProps {
  profiles: ProviderProfile[];
  appFilter: AppKind;
  activeProfileId: string | null;
  selectedId: string | null;
  selectedProfile: ProviderProfile | null;
  editorMode: EditorMode;
  preview: { profileId: string; file: FilePreview } | null;
  busy: boolean;
  onSelectApp: (app: AppKind) => void;
  onNew: () => void;
  onCloseEditor: () => void;
  onSave: (draft: ProviderDraft) => Promise<void>;
  onSelect: (profileId: string) => void;
  onReorder: (orderedIds: string[]) => void;
  onActivate: (profile: ProviderProfile) => void;
  onTogglePreview: (profile: ProviderProfile) => void;
  onEdit: (profile: ProviderProfile) => void;
  onDelete: (profile: ProviderProfile) => void;
  onRequestSwitch: () => void;
}

/** Provider workspace: client tabs, the list with its inline preview, and
 * the dedicated editor view. */
export function ProvidersPage({
  profiles,
  appFilter,
  activeProfileId,
  selectedId,
  selectedProfile,
  editorMode,
  preview,
  busy,
  onSelectApp,
  onNew,
  onCloseEditor,
  onSave,
  onSelect,
  onReorder,
  onActivate,
  onTogglePreview,
  onEdit,
  onDelete,
  onRequestSwitch,
}: ProvidersPageProps) {
  if (editorMode !== null) {
    return (
      <div className="asb-edit-view">
        <div className="asb-edit-header">
          <button
            type="button"
            className="asb-btn-back"
            aria-label="返回供应商列表"
            disabled={busy}
            onClick={onCloseEditor}
          >
            ←
          </button>
          <h2 className="asb-panel-title">
            {editorMode === "new" ? "新建供应商" : "编辑供应商"}
          </h2>
        </div>
        <section className="asb-panel asb-edit-panel">
          <ProviderEditor
            profile={editorMode === "edit" ? selectedProfile : null}
            initialApp={appFilter}
            busy={busy}
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
        <div className="asb-panel-actions">
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
          <Tooltip label="新建供应商" side="bottom">
            <span className="asb-tooltip-anchor">
              <button
                type="button"
                className="asb-btn-plus"
                aria-label="新建供应商"
                disabled={busy}
                onClick={onNew}
              >
                <PlusIcon />
              </button>
            </span>
          </Tooltip>
        </div>
      </div>
      <ProviderList
        profiles={visibleProfiles}
        activeProfileId={activeProfileId}
        selectedId={selectedId}
        openPreviewId={preview?.profileId ?? null}
        onSelect={onSelect}
        onReorder={onReorder}
        onActivate={onActivate}
        onPreview={onTogglePreview}
        onEdit={onEdit}
        onDelete={onDelete}
        renderPreview={() =>
          preview && (
            <section className="asb-preview-inline" aria-label="变更预览">
              <div className="asb-panel-heading">
                <h3 className="asb-panel-title">变更预览</h3>
                <button
                  type="button"
                  className="asb-btn-primary"
                  disabled={busy}
                  onClick={onRequestSwitch}
                >
                  确认切换
                </button>
              </div>
              <PreviewInspector filePreview={preview.file} />
            </section>
          )
        }
      />
    </section>
  );
}
