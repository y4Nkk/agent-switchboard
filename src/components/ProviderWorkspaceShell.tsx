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
import type {
  AppKind,
  ConfigFileStatus,
  LockStatus,
} from "../api/client";
import type { ReactNode, RefCallback } from "react";
import type { ActiveProfileRef } from "../lib/current-provider-name";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { ClientPicker } from "./ClientPicker";
import { DualRelay } from "./DualRelay";
import { GripIcon, PlusIcon } from "./icons";
import { Tooltip } from "./Tooltip";
import { WorkspaceHeader } from "./WorkspaceHeader";
import "../styles/base/provider-workspace.css";

interface ProviderWorkspaceShellProps {
  /** Distinguishes the two client views for assistive technology; the visible
   * heading and toolbar are identical on both. */
  ariaLabel: string;
  app: AppKind;
  routeContentRef: RefCallback<HTMLElement>;
  /** True while this workspace is being replaced by a tab switch: it stays
   * mounted for one exit cycle, overlays the entering workspace, and ignores
   * pointers and assistive technology until the parent unmounts it. */
  leaving: boolean;
  onSelectApp: (app: AppKind) => void;
  busy: boolean;
  statuses: ConfigFileStatus[] | null;
  /** Every stored profile across both clients; the route cards' sole lookup
   * source for the active profile's name and website. */
  profiles: readonly ActiveProfileRef[];
  locks: Partial<Record<AppKind, LockStatus>>;
  onImport: () => void;
  onConvert: () => void;
  onNew: () => void;
  /** App-specific actions appended between 导入 / 导出 and 新建供应商. */
  extraActions?: ReactNode;
  children: ReactNode;
}

/** The page heading leads, while connection cards keep their existing visual expression. */
export function ProviderWorkspaceShell({
  ariaLabel,
  app,
  routeContentRef,
  leaving,
  onSelectApp,
  busy,
  statuses,
  profiles,
  locks,
  onImport,
  onConvert,
  onNew,
  extraActions,
  children,
}: ProviderWorkspaceShellProps) {
  const { t } = useI18n();
  return (
    <section
      className="asb-provider-workspace"
      data-client-route={app}
      data-client-route-leaving={leaving || undefined}
      inert={leaving || undefined}
      aria-hidden={leaving || undefined}
      aria-label={ariaLabel}
    >
        <WorkspaceHeader
          title={t("nav.page.providers")}
          primary={
            <ClientPicker
              app={app}
              onChange={onSelectApp}
              disabled={busy}
              label={t("providers.workspace.clientPicker")}
            />
          }
          primaryActions={
            <>
              <Button variant="secondary" disabled={busy} onClick={onImport}>
                {t("providers.workspace.importExport")}
              </Button>
              {extraActions}
              <Button variant="secondary" disabled={busy} data-provider-conversion-app={app} onClick={onConvert}>
                {t("providers.conversion.open")}
              </Button>
              <Button variant="plus" disabled={busy} onClick={onNew}>
                <PlusIcon />
                {t("providers.workspace.newProvider")}
              </Button>
            </>
          }
        />
      <DualRelay statuses={statuses} profiles={profiles} locks={locks} />
      <section ref={routeContentRef} className="asb-provider-list" aria-label={t("providers.list.aria")}>
        {children}
      </section>
    </section>
  );
}

interface SortableProviderRowsProps {
  ids: string[];
  /** Persists a new display order for the visible rows. */
  onReorder?: (orderedIds: string[]) => void;
  emptyLabel?: string;
  ariaLabel?: string;
  children: ReactNode;
}

/** The one sortable rows container, including the shared empty state. */
export function SortableProviderRows({
  ids,
  onReorder,
  emptyLabel,
  ariaLabel,
  children,
}: SortableProviderRowsProps) {
  const { t } = useI18n();
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = ids.indexOf(String(active.id));
    const newIndex = ids.indexOf(String(over.id));
    if (oldIndex === -1 || newIndex === -1) return;
    onReorder?.(arrayMove(ids, oldIndex, newIndex));
  };
  if (ids.length === 0) {
    return (
      <div className="asb-empty-state">
        <span className="asb-empty-state-icon" aria-hidden="true">
          <PlusIcon />
        </span>
        <h3 className="asb-section-title">{emptyLabel ?? t("providers.list.empty")}</h3>
      </div>
    );
  }
  return (
    <ul className="asb-rows" role="list" aria-label={ariaLabel ?? t("providers.list.aria")}>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          {children}
        </SortableContext>
      </DndContext>
    </ul>
  );
}

interface ProviderRowShellProps {
  id: string;
  name: string;
  active: boolean;
  confirmationOpen: boolean;
  sortable: boolean;
  /** The configured model, rendered in the row's dedicated model zone. */
  model?: string | null;
  modelTooltip?: string;
  /** The provider host or official route, rendered in the connection zone. */
  endpoint?: ReactNode;
  /** Usage or account state, rendered beneath the fixed primary data rail. */
  summary?: ReactNode;
  /** Primary action for an inactive profile (启用). */
  primaryAction?: ReactNode;
  /** Secondary explicit action, such as official re-login. */
  secondaryAction?: ReactNode;
  /** Secondary icon actions sharing the row's hover/focus visibility rule. */
  actions?: ReactNode;
  /** Expansion blocks rendered below the data row. */
  children?: ReactNode;
}

/**
 * One provider row keeps identity, model, connection, and actions on a
 * fixed scan rail. Its optional full-width summary lives below that rail,
 * leaving the model value as an unambiguous single-line fact.
 */
export function ProviderRowShell({
  id,
  name,
  active,
  confirmationOpen,
  sortable,
  model,
  modelTooltip,
  endpoint,
  summary,
  primaryAction,
  secondaryAction,
  actions,
  children,
}: ProviderRowShellProps) {
  const { t } = useI18n();
  const { setNodeRef, attributes, listeners, transform, transition, isDragging } = useSortable({
    id,
    disabled: !sortable,
  });
  const initial = name.trim().charAt(0).toUpperCase() || "?";
  const hasSummary = summary !== undefined && summary !== null;
  const dragLabel = t("providers.row.dragAria", { name });
  const modelValue = <span className="asb-row-model-value">{model ?? t("providers.label.defaultModel")}</span>;
  const modelHint = modelTooltip ?? model;
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={`asb-row-item${active ? " is-live" : ""}${isDragging ? " is-dragging" : ""}${confirmationOpen ? " is-confirming" : ""}`}
    >
      <div className="asb-row-line">
        {sortable ? (
          <Tooltip label={dragLabel}>
            <Button
              variant="unstyled"
              className="asb-row-grip asb-row-action"
              aria-label={dragLabel}
              {...attributes}
              {...listeners}
            >
              <GripIcon />
            </Button>
          </Tooltip>
        ) : <span className="asb-row-grip-spacer" aria-hidden="true" />}
        <div className="asb-row">
          <span className="asb-avatar" aria-hidden="true">{initial}</span>
          <span className="asb-row-identity-copy">
            <Tooltip asChild label={name}><span className="asb-row-name">{name}</span></Tooltip>
            {active && <span className="asb-row-applied">{t("providers.label.applied")}</span>}
          </span>
        </div>
        <span className="asb-row-model">
          {modelHint ? <Tooltip asChild label={modelHint}>{modelValue}</Tooltip> : modelValue}
        </span>
        <span className="asb-row-endpoint">{endpoint}</span>
        <span className="asb-row-controls asb-row-action">
          {primaryAction}
          {secondaryAction}
          {actions && <span className="asb-iconcluster" role="group" aria-label={t("providers.row.actionsAria", { name })}>{actions}</span>}
        </span>
      </div>
      {hasSummary && <div className="asb-row-summary">{summary}</div>}
      {children}
    </li>
  );
}
