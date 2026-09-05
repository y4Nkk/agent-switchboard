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
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ConnectivityIcon,
  EditIcon,
  EyeOffIcon,
  GripIcon,
  MoreIcon,
  PlayIcon,
  PreviewIcon,
  TrashIcon,
  UsageIcon,
} from "./icons";
import { ProbeFeedback, useEndpointProbe } from "./ProbePanel";
import { CodexOfficialQuotaPanel } from "./CodexOfficialQuotaPanel";
import { Button } from "./Button";
import { OfficialLoginPanel } from "./OfficialLoginPanel";
import { StarlightLayer } from "./experience/StarlightLayer";
import { ProviderUsagePanel } from "./ProviderUsagePanel";
import { useProviderUsage, type ProviderUsage } from "./use-provider-usage";
import { formatUsageSummary } from "../lib/usage-format";
import { cx } from "@/utils/cx";
import { Tooltip } from "./Tooltip";

interface Props {
  profiles: ProviderProfile[];
  /** Profile id the live file actually matches, when the app can tell. */
  activeProfileId: string | null;
  /** Model read from the displayed client's user-level configuration file. */
  userConfigModel: string | null;
  selectedId: string | null;
  /** Profile whose preview is currently unfolded under the list. */
  openPreviewId?: string | null;
  /** Persisted profile ids whose usage panel is collapsed; every other
   * configured panel stays expanded. */
  collapsedUsageIds?: string[];
  onSelect: (id: string) => void;
  /** Persists a new display order for the visible client's profiles. */
  onReorder?: (orderedIds: string[]) => void;
  /** Persists the flipped usage-panel state for the profile. */
  onToggleUsage?: (profile: ProviderProfile) => void;
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
  userConfigModel: string | null;
  selected: boolean;
  previewOpen: boolean;
  usageOpen: boolean;
  /** Official Codex rows: whether the subscription-quota ledger is unfolded,
   * persisted through the same collapsed-usage owner as `usageOpen`. */
  quotaOpen: boolean;
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

function ConfiguredProviderRow(props: RowProps) {
  const usage = useProviderUsage(props.profile);
  return <ProviderRow {...props} usage={usage} />;
}

/** Provider cards use the app's compact routing layout. The grip handle is
 * the only drag affordance; the row body stays a plain click-to-select
 * target. */
function ProviderRow({
  profile,
  active,
  userConfigModel,
  selected,
  previewOpen,
  usageOpen,
  quotaOpen,
  sortable,
  onSelect,
  onToggleUsage,
  onActivate,
  onPreview,
  onEdit,
  onConfigureUsage,
  onDelete,
  renderPreview,
  usage,
}: RowProps & { usage?: ProviderUsage }) {
  const { setNodeRef, attributes, listeners, transform, transition, isDragging } = useSortable({
    id: profile.id,
    disabled: !sortable,
  });
  const initial = profile.name.trim().charAt(0).toUpperCase() || "?";
  const baseUrl = profile.baseUrl;
  const websiteUrl = profile.websiteUrl;
  const probe = useEndpointProbe(baseUrl ?? null);
  const [probeOpen, setProbeOpen] = useState(false);
  const [reloginOpen, setReloginOpen] = useState(false);
  /** Bumped on each completed re-login so the quota panel re-queries. */
  const [quotaNonce, setQuotaNonce] = useState(0);
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLSpanElement>(null);
  const moreTriggerRef = useRef<HTMLButtonElement>(null);
  const official = profile.routeMode === "official";
  const officialQuota = official && profile.app === "codex";
  const displayedModel = active ? userConfigModel : profile.model;
  const modelText = active
    ? `当前用户级配置模型：${displayedModel ?? "默认模型"}`
    : displayedModel;
  const hasUsageQuery = profile.usageQuery !== null && profile.usageQuery !== undefined;
  const usageLabel = hasUsageQuery
    ? usageOpen
      ? `收起 ${profile.name} 用量`
      : `查看 ${profile.name} 用量`
    : `配置 ${profile.name} 用量`;
  const quotaLabel = quotaOpen
    ? `收起 ${profile.name} 订阅额度`
    : `查看 ${profile.name} 订阅额度`;
  const hasProbeFeedback = probe.result !== null || probe.error !== null;
  const probeFeedbackId = `provider-probe-${profile.id}`;
  const probeVisible = probeOpen && hasProbeFeedback;
  const probeLabel = probe.busy
    ? `正在测试 ${profile.name} 连通性`
    : probeVisible
      ? `收起 ${profile.name} 连通性结果`
      : `测试 ${profile.name} 连通性`;
  const hasClusterActions = Boolean(
    baseUrl ||
      hasUsageQuery ||
      officialQuota ||
      (!official && onConfigureUsage) ||
      onPreview ||
      onEdit ||
      onDelete,
  );

  // Destructive entries live behind the three-dot trigger; the menu follows
  // the app's popup idioms (FontPicker): outside pointerdown and Escape close.
  useEffect(() => {
    if (!moreOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!moreRef.current?.contains(event.target as Node)) setMoreOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [moreOpen]);
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={`asb-row-item${active ? " is-live" : ""}${selected ? " is-selected" : ""}${isDragging ? " is-dragging" : ""}${previewOpen ? " is-previewing" : ""}`}
    >
      <div className="asb-row-line">
      <StarlightLayer active={selected} variant="warm" />
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
          {(modelText || websiteUrl || official || (usage && !usageOpen)) && (
            <span className={cx("asb-row-meta", usage && !usageOpen && "asb-row-meta-with-usage")}>
              {modelText}
              {modelText && (websiteUrl || official) && " · "}
              {websiteUrl ? (
                <a
                  className="asb-row-host"
                  href={websiteUrl}
                  title={websiteUrl}
                  onClick={(event) => {
                    // wry blocks webview new-window requests; the opener plugin
                    // routes the URL to the system browser instead.
                    event.preventDefault();
                    void openUrl(websiteUrl);
                  }}
                >
                  {hostLabel(websiteUrl)}
                </a>
              ) : official ? (
                <span>官方登录</span>
              ) : null}
              {usage && !usageOpen && (
                <span aria-label={`${profile.name} 用量摘要`} title={usage.error ?? undefined}>
                  {(modelText || websiteUrl || official) && " · "}
                  {usage.data ? formatUsageSummary(usage.data) : usage.error ? "用量查询失败" : "用量读取中…"}
                  {usage.data && usage.error && "（更新失败，显示上次读数）"}
                  {usage.data && usage.querying && "（更新中…）"}
                </span>
              )}
            </span>
          )}
        </span>
      </button>
      {onActivate && !active && (
        <Tooltip label={`启用 ${profile.name}`}>
          <Button
            variant="primary"
            className="asb-row-activate"
            aria-label={`启用 ${profile.name}`}
            onClick={() => onActivate(profile)}
          >
            <PlayIcon size={15} />
            启用
          </Button>
        </Tooltip>
      )}
      {active && <span className="asb-pill-status">使用中</span>}
      {official && (
        <Tooltip label={reloginOpen ? `收起 ${profile.name} 登录` : `重新登录 ${profile.name}`}>
          <Button
            variant="secondary"
            className={`asb-row-activate${reloginOpen ? " is-active" : ""}`}
            aria-label={reloginOpen ? `收起 ${profile.name} 登录` : `重新登录 ${profile.name}`}
            aria-expanded={reloginOpen}
            onClick={() => setReloginOpen((open) => !open)}
          >
            {reloginOpen ? "收起登录" : "重新登录"}
          </Button>
        </Tooltip>
      )}
      {hasClusterActions && (
        <span className="asb-iconcluster" role="group" aria-label={`${profile.name} 操作`}>
          {onEdit && (
            <Tooltip label={`编辑 ${profile.name}`}>
              <Button
                variant="icon"
                aria-label={`编辑 ${profile.name}`}
                onClick={() => onEdit(profile)}
              >
                <EditIcon size={20} />
              </Button>
            </Tooltip>
          )}
          {onPreview && (
            <Tooltip label={previewOpen ? `收起 ${profile.name} 预览` : `预览 ${profile.name} 变更`}>
              <Button
                variant="icon"
                className={previewOpen ? "is-active" : undefined}
                aria-label={previewOpen ? `收起 ${profile.name} 预览` : `预览 ${profile.name} 变更`}
                aria-expanded={previewOpen}
                onClick={() => onPreview(profile)}
              >
                {previewOpen ? <EyeOffIcon size={20} /> : <PreviewIcon size={20} />}
              </Button>
            </Tooltip>
          )}
          {baseUrl && (
            <Tooltip label={probeLabel}>
              <Button
                variant="icon"
                className={probeVisible ? "is-active" : undefined}
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
                <ConnectivityIcon size={20} />
              </Button>
            </Tooltip>
          )}
          {officialQuota && (
            <Tooltip label={quotaLabel}>
              <Button
                variant="icon"
                className={quotaOpen ? "is-active" : undefined}
                aria-label={quotaLabel}
                aria-controls={`codex-official-quota-${profile.id}`}
                aria-expanded={quotaOpen}
                onClick={() => onToggleUsage(profile)}
              >
                <UsageIcon size={20} />
              </Button>
            </Tooltip>
          )}
          {!official && (hasUsageQuery || onConfigureUsage) && (
            <Tooltip label={usageLabel}>
              <Button
                variant="icon"
                className={usageOpen ? "is-active" : undefined}
                aria-label={usageLabel}
                aria-controls={hasUsageQuery ? `provider-usage-${profile.id}` : undefined}
                aria-expanded={hasUsageQuery ? usageOpen : undefined}
                onClick={() => {
                  if (hasUsageQuery) onToggleUsage(profile);
                  else onConfigureUsage?.(profile);
                }}
              >
                <UsageIcon size={20} />
              </Button>
            </Tooltip>
          )}
          {onDelete && (
            <span
              className="asb-row-more"
              ref={moreRef}
              onKeyDown={(event) => {
                // Escape is handled on the wrapper so it closes the menu even
                // while focus stays on the trigger, like FontPicker.
                if (event.key === "Escape" && moreOpen) {
                  event.preventDefault();
                  setMoreOpen(false);
                  moreTriggerRef.current?.focus();
                }
              }}
            >
              <Tooltip label={`更多 ${profile.name} 操作`}>
                <Button
                  variant="icon"
                  ref={moreTriggerRef}
                  className={moreOpen ? "is-active" : undefined}
                  aria-label={`更多 ${profile.name} 操作`}
                  aria-haspopup="menu"
                  aria-expanded={moreOpen}
                  onClick={() => setMoreOpen((open) => !open)}
                >
                  <MoreIcon size={20} />
                </Button>
              </Tooltip>
              {moreOpen && (
                <span className="asb-row-menu" role="menu" aria-label={`${profile.name} 更多操作`}>
                  <button
                    type="button"
                    role="menuitem"
                    className="asb-row-menu-item"
                    aria-label={`删除 ${profile.name}`}
                    onClick={() => {
                      setMoreOpen(false);
                      onDelete(profile);
                    }}
                  >
                    <TrashIcon size={15} />
                    删除
                  </button>
                </span>
              )}
            </span>
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
      {usageOpen && usage && (
        <ProviderUsagePanel
          id={`provider-usage-${profile.id}`}
          profile={profile}
          usage={usage}
          onConfigure={onConfigureUsage}
        />
      )}
      {reloginOpen && official && (
        <OfficialLoginPanel
          app={profile.app}
          onFinished={(completed) => {
            if (completed) setQuotaNonce((nonce) => nonce + 1);
          }}
        />
      )}
      {officialQuota && quotaOpen && (
        <CodexOfficialQuotaPanel
          key={`codex-official-quota-${profile.id}-${quotaNonce}`}
          id={`codex-official-quota-${profile.id}`}
          profileId={profile.id}
          profileName={profile.name}
        />
      )}
      {previewOpen && renderPreview && renderPreview(profile)}
    </li>
  );
}

export function ProviderList({
  profiles,
  activeProfileId,
  userConfigModel,
  selectedId,
  openPreviewId,
  collapsedUsageIds = [],
  onSelect,
  onReorder,
  onToggleUsage,
  onActivate,
  onPreview,
  onEdit,
  onConfigureUsage,
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
          {profiles.map((profile) => {
            const Row = profile.usageQuery ? ConfiguredProviderRow : ProviderRow;
            return <Row
              key={profile.id}
              profile={profile}
              active={profile.id === activeProfileId}
              userConfigModel={userConfigModel}
              selected={selectedId === profile.id}
              previewOpen={profile.id === openPreviewId}
              usageOpen={Boolean(profile.usageQuery) && !collapsedUsageIds.includes(profile.id)}
              quotaOpen={
                profile.routeMode === "official" &&
                profile.app === "codex" &&
                !collapsedUsageIds.includes(profile.id)
              }
              sortable={Boolean(onReorder)}
              onSelect={onSelect}
              onToggleUsage={(toggled) => onToggleUsage?.(toggled)}
              onActivate={onActivate}
              onPreview={onPreview}
              onEdit={onEdit}
              onConfigureUsage={onConfigureUsage}
              onDelete={onDelete}
              renderPreview={renderPreview}
            />;
          })}
        </SortableContext>
      </DndContext>
    </ul>
  );
}
