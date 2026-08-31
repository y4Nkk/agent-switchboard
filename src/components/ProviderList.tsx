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
import type { ReactNode } from "react";
import { EditIcon, EyeOffIcon, GripIcon, PlayIcon, PreviewIcon, TrashIcon } from "./icons";
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
  sortable: boolean;
  onSelect: (id: string) => void;
  onActivate?: (profile: ProviderProfile) => void;
  onPreview?: (profile: ProviderProfile) => void;
  onEdit?: (profile: ProviderProfile) => void;
  onDelete?: (profile: ProviderProfile) => void;
  renderPreview?: (profile: ProviderProfile) => ReactNode;
}

/** CC Switch-style provider cards: avatar tile, name over a link-colored
 * detail line, a status pill for the live-matched row, and an icon-action
 * cluster in a pill container (user-directed replica, DESIGN.md §8). The
 * grip handle is the only drag affordance; the row body stays a plain
 * click-to-select target. */
function ProviderRow({
  profile,
  active,
  selected,
  previewOpen,
  sortable,
  onSelect,
  onActivate,
  onPreview,
  onEdit,
  onDelete,
  renderPreview,
}: RowProps) {
  const { setNodeRef, attributes, listeners, transform, transition, isDragging } = useSortable({
    id: profile.id,
    disabled: !sortable,
  });
  const initial = profile.name.trim().charAt(0).toUpperCase() || "?";
  const baseUrl = profile.baseUrl;
  // Valid profiles always carry an endpoint; the fallback only guards
  // pre-migration shapes.
  const detail = baseUrl ? hostLabel(baseUrl) : "自定义服务";
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={`asb-row-item${selected ? " is-selected" : ""}${active ? " is-live" : ""}${
        isDragging ? " is-dragging" : ""
      }`}
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
      {(onPreview || onEdit || onDelete) && (
        <span className="asb-iconcluster" role="group" aria-label={`${profile.name} 操作`}>
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
          {onEdit && (
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
  onDelete,
  renderPreview,
}: Props) {
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
              sortable={Boolean(onReorder)}
              onSelect={onSelect}
              onActivate={onActivate}
              onPreview={onPreview}
              onEdit={onEdit}
              onDelete={onDelete}
              renderPreview={renderPreview}
            />
          ))}
        </SortableContext>
      </DndContext>
    </ul>
  );
}
