import { CSS } from "@dnd-kit/utilities";
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import type { ProviderProfile } from "../api/client";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState, type ReactNode } from "react";
import {
  ConnectivityIcon,
  EditIcon,
  EyeOffIcon,
  GripIcon,
  PlayIcon,
  PreviewIcon,
  TrashIcon,
  UsageIcon,
} from "./icons";
import { ProbeFeedback, useEndpointProbe } from "./ProbePanel";
import { ProviderUsagePanel } from "./ProviderUsagePanel";
import { Tooltip } from "./Tooltip";

interface Props {
  profiles: ProviderProfile[];
  /** Profile id the live file actually matches, when the app can tell. */
  activeProfileId: string | null;
  selectedId: string | null;
  /** Profile whose preview is currently unfolded under the list. */
  openPreviewId?: string | null;
  onSelect: (id: string) => void;
  /** Persists a new display order for the visible client's profiles. */
  onReorder?: (orderedIds: string[]) => void;
  /** Opens the preview panel for the profile; the write itself still needs
   * the explicit confirm step (user decision 2026-08-28). */
  onActivate?: (profile: ProviderProfile) => void;
  onPreview?: (profile: ProviderProfile) => void;
  onEdit?: (profile: ProviderProfile) => void;
  /** Opens the dedicated usage-query workspace for this profile. */
  onConfigureUsage?: (profile: ProviderProfile) => void;
  onDelete?: (profile: ProviderProfile) => void;
  /** Expansion content rendered inside the previewed row's own card, under
   * the row line. Ownership stays with the caller; the list only places it. */
  renderPreview?: (profile: ProviderProfile) => ReactNode;
}

function hostLabel(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

interface RowProps {
  profile: ProviderProfile;
  active: boolean;
  selected: boolean;
  previewOpen: boolean;
  usageOpen: boolean;
  sortable: boolean;
  onSelect: (id: string) => void;
  onToggleUsage: (profile: ProviderProfile) => void;
  onActivate?: (profile: ProviderProfile) => void;
  onPreview?: (profile: ProviderProfile) => void;
  onEdit?: (profile: ProviderProfile) => void;
  onConfigureUsage?: (profile: ProviderProfile) => void;
  onDelete?: (profile: ProviderProfile) => void;
  renderPreview?: (profile: ProviderProfile) => ReactNode;
}

/** Provider cards use the app's compact routing layout. The grip handle is
 * the only drag affordance; the row body stays a plain click-to-select
 * target. */
function ProviderRow({
  profile,
  active,
  selected,
  previewOpen,
  usageOpen,
  sortable,
  onSelect,
  onToggleUsage,
  onActivate,
  onPreview,
  onEdit,
  onConfigureUsage,
  onDelete,
  renderPreview,
}: RowProps) {
  const { setNodeRef, attributes, listeners, transform, transition, isDragging } = useSortable({
    id: profile.id,
    disabled: !sortable,
  });
  const initial = profile.name.trim().charAt(0).toUpperCase() || "?";
  const baseUrl = profile.baseUrl;
  const probe = useEndpointProbe(baseUrl ?? null);
  const [probeOpen, setProbeOpen] = useState(false);
  const official = profile.routeMode === "official";
  const detail = baseUrl ? hostLabel(baseUrl) : official ? "官方登录" : "自定义服务";
  const hasUsageQuery = profile.usageQuery !== null && profile.usageQuery !== undefined;
  const usageLabel = hasUsageQuery
    ? usageOpen
      ? `收起 ${profile.name} 用量`
      : `查看 ${profile.name} 用量`
    : `配置 ${profile.name} 用量`;
  const hasProbeFeedback = probe.result !== null || probe.error !== null;
  const probeFeedbackId = `provider-probe-${profile.id}`;
  const probeVisible = probeOpen && hasProbeFeedback;
  const probeLabel = probe.busy
    ? `正在测试 ${profile.name} 连通性`
    : probeVisible
      ? `收起 ${profile.name} 连通性结果`
      : `测试 ${profile.name} 连通性`;
  const hasActions = Boolean(baseUrl || hasUsageQuery || (!official && onConfigureUsage) || onPreview || (!official && onEdit) || onDelete);
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={`asb-row-item${active ? " is-live" : ""}${isDragging ? " is-dragging" : ""}${previewOpen ? " is-previewing" : ""}`}
    >
      <div className="asb-row-line">
      {sortable && (
        <Tooltip label={`拖动调整 ${profile.name} 的顺序`}>
          <button
            type="button"
            className="asb-row-grip"
            aria-label={`拖动调整 ${profile.name} 的顺序`}
            {...attributes}
            {...listeners}
          >
            <GripIcon />
          </button>
        </Tooltip>
      )}
      <button
        type="button"
        role="option"
        aria-selected={selected}
        className="asb-row"
        onClick={() => onSelect(profile.id)}
      >
        <span className="asb-avatar" aria-hidden="true">
          {initial}
        </span>
        <span className="asb-row-main">
          <span className="asb-row-name">{profile.name}</span>
          {baseUrl ? (
            <a
              className="asb-row-meta is-url"
              href={baseUrl}
              title={baseUrl}
              onClick={(event) => {
                // wry blocks webview new-window requests; the opener plugin
                // routes the URL to the system browser instead.
                event.preventDefault();
                void openUrl(baseUrl);
              }}
            >
              {hostLabel(baseUrl)}
            </a>
          ) : (
            <span className="asb-row-meta">{detail}</span>
          )}
          {profile.model && <span className="asb-row-meta">{profile.model}</span>}
        </span>
      </button>
      {onActivate && !active && (
        <Tooltip label={`启用 ${profile.name}`}>
          <button
            type="button"
            className="asb-btn-primary asb-row-activate"
            aria-label={`启用 ${profile.name}`}
            onClick={() => onActivate(profile)}
          >
            <PlayIcon size={15} />
            启用
          </button>
        </Tooltip>
      )}
      {active && <span className="asb-pill-status">使用中</span>}
      {hasActions && (
        <span className="asb-iconcluster" role="group" aria-label={`${profile.name} 操作`}>
          {onEdit && !official && (
            <Tooltip label={`编辑 ${profile.name}`}>
              <button
                type="button"
                className="asb-btn-icon"
                aria-label={`编辑 ${profile.name}`}
                onClick={() => onEdit(profile)}
              >
                <EditIcon />
              </button>
            </Tooltip>
          )}
          {onPreview && (
            <Tooltip label={previewOpen ? `收起 ${profile.name} 预览` : `预览 ${profile.name} 变更`}>
              <button
                type="button"
                className={`asb-btn-icon${previewOpen ? " is-active" : ""}`}
                aria-label={previewOpen ? `收起 ${profile.name} 预览` : `预览 ${profile.name} 变更`}
                aria-expanded={previewOpen}
                onClick={() => onPreview(profile)}
              >
                {previewOpen ? <EyeOffIcon /> : <PreviewIcon />}
              </button>
            </Tooltip>
          )}
          {baseUrl && (
            <Tooltip label={probeLabel}>
              <button
                type="button"
                className={`asb-btn-icon${probeVisible ? " is-active" : ""}`}
                aria-label={probeLabel}
                aria-busy={probe.busy}
                aria-controls={probeFeedbackId}
                aria-expanded={probeVisible}
                aria-describedby={probeVisible ? probeFeedbackId : undefined}
                disabled={probe.busy}
                onClick={() => {
                  if (probeVisible) {
                    setProbeOpen(false);
                    return;
                  }
                  setProbeOpen(true);
                  void probe.run();
                }}
              >
                <ConnectivityIcon />
              </button>
            </Tooltip>
          )}
          {!official && (hasUsageQuery || onConfigureUsage) && (
            <Tooltip label={usageLabel}>
              <button
                type="button"
                className={`asb-btn-icon${usageOpen ? " is-active" : ""}`}
                aria-label={usageLabel}
                aria-controls={hasUsageQuery ? `provider-usage-${profile.id}` : undefined}
                aria-expanded={hasUsageQuery ? usageOpen : undefined}
                onClick={() => {
                  if (hasUsageQuery) onToggleUsage(profile);
                  else onConfigureUsage?.(profile);
                }}
              >
                <UsageIcon />
              </button>
            </Tooltip>
          )}
          {onDelete && (
            <Tooltip label={`删除 ${profile.name}`}>
              <button
                type="button"
                className="asb-btn-icon"
                aria-label={`删除 ${profile.name}`}
                onClick={() => onDelete(profile)}
              >
                <TrashIcon />
              </button>
            </Tooltip>
          )}
        </span>
      )}
      </div>
      {probeVisible && (
        <ProbeFeedback
          id={probeFeedbackId}
          className="asb-provider-probe-feedback"
          result={probe.result}
          error={probe.error}
        />
      )}
      {usageOpen && profile.usageQuery && (
        <ProviderUsagePanel
          id={`provider-usage-${profile.id}`}
          profile={profile}
          query={profile.usageQuery}
          onConfigure={onConfigureUsage}
        />
      )}
      {previewOpen && renderPreview && renderPreview(profile)}
    </li>
  );
}

export function ProviderList({
  profiles,
  activeProfileId,
  selectedId,
  openPreviewId,
  onSelect,
  onReorder,
  onActivate,
  onPreview,
  onEdit,
  onConfigureUsage,
  onDelete,
  renderPreview,
}: Props) {
  const [collapsedUsageIds, setCollapsedUsageIds] = useState<string[]>([]);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const ids = profiles.map((profile) => profile.id);
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = ids.indexOf(String(active.id));
    const newIndex = ids.indexOf(String(over.id));
    if (oldIndex === -1 || newIndex === -1) return;
    onReorder?.(arrayMove(ids, oldIndex, newIndex));
  };
  const toggleUsage = (profile: ProviderProfile) => {
    if (!profile.usageQuery) return;
    setCollapsedUsageIds((current) =>
      current.includes(profile.id)
        ? current.filter((id) => id !== profile.id)
        : [...current, profile.id],
    );
  };
  if (profiles.length === 0) {
    return <p className="asb-empty">尚无供应商</p>;
  }
  return (
    <ul className="asb-rows" role="listbox" aria-label="供应商列表">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          {profiles.map((profile) => (
            <ProviderRow
              key={profile.id}
              profile={profile}
              active={profile.id === activeProfileId}
              selected={selectedId === profile.id}
              previewOpen={profile.id === openPreviewId}
              usageOpen={Boolean(profile.usageQuery) && !collapsedUsageIds.includes(profile.id)}
              sortable={Boolean(onReorder)}
              onSelect={onSelect}
              onToggleUsage={toggleUsage}
              onActivate={onActivate}
              onPreview={onPreview}
              onEdit={onEdit}
              onConfigureUsage={onConfigureUsage}
              onDelete={onDelete}
              renderPreview={renderPreview}
            />
          ))}
        </SortableContext>
      </DndContext>
    </ul>
  );
}
