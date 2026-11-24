import { uiMessage } from "../../i18n/errors";
import { useEffect, useId, useState, type ComponentType } from "react";
import * as api from "../../api/codex-project-plans";
import type { MessageKey, TFunction } from "../../i18n";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { FactList } from "../FactList";
import { Input } from "../Input";
import { ClientManagementModule } from "../client-management/ClientManagementModule";
import { McpKindIcon, ProviderKindIcon, ScrollTextIcon, SkillKindIcon } from "../icons";
import type { CodexOperations } from "./operations";

const ACTION_LABELS: Record<api.CodexProjectApplyStep["action"], MessageKey> = {
  switch: "codex.plans.actionSwitch", enable: "codex.plans.actionEnable",
  disable: "codex.plans.actionDisable", activate: "codex.plans.actionActivate",
};
const KIND_LABELS: Record<api.CodexProjectApplyStep["kind"], MessageKey> = {
  provider: "codex.plans.kindProvider", mcp: "codex.plans.kindMcp",
  skill: "codex.plans.kindSkill", prompt: "codex.plans.kindPrompt",
};

function stepText(step: api.CodexProjectApplyStep, t: TFunction): string {
  return t("codex.plans.stepText", {
    kind: t(KIND_LABELS[step.kind]),
    action: t(ACTION_LABELS[step.action]),
    label: step.label,
  });
}

function projectPlanName(view: api.CodexProjectPlansView, id: string): string {
  return view.plans.find((plan) => plan.id === id)?.name
    ?? view.providers.find((option) => option.id === id)?.name
    ?? view.prompts.find((option) => option.id === id)?.name
    ?? view.bindings.find((binding) => binding.definitionId === id)?.name
    ?? id;
}

type SlotKind = "provider" | "mcp" | "skill" | "prompt";
/** `null` is a slot a scenario never captured; restoring leaves it untouched. */
type SlotValues = Record<SlotKind, string | null>;

/** The four things a scenario holds, in the order the interface shows them. */
const SLOT_KINDS: ReadonlyArray<{ kind: SlotKind; label: MessageKey; Icon: ComponentType<{ size?: number }> }> = [
  { kind: "provider", label: "codex.plans.kindProvider", Icon: ProviderKindIcon },
  { kind: "mcp", label: "codex.plans.kindMcp", Icon: McpKindIcon },
  { kind: "skill", label: "codex.plans.kindSkill", Icon: SkillKindIcon },
  { kind: "prompt", label: "codex.plans.kindPrompt", Icon: ScrollTextIcon },
];

/** The slots as one FactList: a label with its glyph above the current or the
 * captured value. */
function SlotStrip({ values }: { values: SlotValues }) {
  const { t } = useI18n();
  return <div className="asb-slot-strip">
    <FactList inline rows={SLOT_KINDS.map(({ kind, label, Icon }) => ({
      label: <><span className="asb-slot-glyph" aria-hidden="true"><Icon /></span>{t(label)}</>,
      value: values[kind] ?? <span className="asb-slot-empty">{t("codex.plans.slotEmpty")}</span>,
    }))} />
  </div>;
}

function liveSlots(view: api.CodexProjectPlansView, t: TFunction): SlotValues {
  const enabled = (kind: "mcp" | "skill") =>
    view.bindings.filter((binding) => binding.kind === kind && binding.enabled).length;
  return {
    provider: view.activeProviderId ? projectPlanName(view, view.activeProviderId) : t("codex.plans.none"),
    mcp: t("codex.plans.enabledCount", { count: enabled("mcp") }),
    skill: t("codex.plans.enabledCount", { count: enabled("skill") }),
    prompt: view.activePromptId ? projectPlanName(view, view.activePromptId) : t("codex.plans.none"),
  };
}

function scenarioSlots(view: api.CodexProjectPlansView, plan: api.CodexProjectPlan, t: TFunction): SlotValues {
  const count = (ids: string[]) => t("codex.plans.itemCount", { count: ids.length });
  return {
    provider: plan.slot.providers === null ? null
      : plan.slot.providers ? projectPlanName(view, plan.slot.providers) : t("codex.plans.noActiveProvider"),
    mcp: plan.slot.mcp === null ? null : count(plan.slot.mcp),
    skill: plan.slot.skills === null ? null : count(plan.slot.skills),
    prompt: plan.slot.prompts === null ? null
      : plan.slot.prompts ? projectPlanName(view, plan.slot.prompts) : t("codex.plans.noActivePrompt"),
  };
}

/** Reserves the stage's footprint until the scenarios are read. */
function PlanSkeleton() {
  const { t } = useI18n();
  return <div className="asb-tool-stage">
    <div className="asb-plan-skeleton" role="status" aria-label={t("codex.loading")}>
      <div className="asb-plan-skeleton-slots" aria-hidden="true">
        <div className="asb-skeleton" /><div className="asb-skeleton" /><div className="asb-skeleton" /><div className="asb-skeleton" />
      </div>
      <div className="asb-skeleton" aria-hidden="true" />
    </div>
  </div>;
}

/** What Codex uses right now, and the action that saves it as a scenario. */
function CurrentStateStage({ view, operations, newName, setNewName, reset, onCreated }: {
  view: api.CodexProjectPlansView; operations: CodexOperations; newName: string;
  setNewName: (name: string) => void; reset: (next: api.CodexProjectPlansView) => void;
  onCreated: (plan: api.CodexProjectPlan | null) => void;
}) {
  const { run, busy, changed } = operations;
  const { t } = useI18n();
  return <div className="asb-tool-stage">
    <h4 className="asb-group-title asb-plan-stage-title">{t("codex.plans.currentState")}</h4>
    <SlotStrip values={liveSlots(view, t)} />
    <div className="asb-plan-save">
      <label className="asb-field is-medium"><span>{t("codex.plans.newNameLabel")}</span>
        <Input value={newName} disabled={busy} onChange={(event) => setNewName(event.target.value)} /></label>
      <Button variant="primary" disabled={busy || !newName.trim()} onClick={() => void run(async () => {
        const next = await api.createCodexProjectPlan(newName, view.revision);
        reset(next);
        onCreated(next.plans[next.plans.length - 1] ?? null);
        changed(uiMessage("codex.plans.currentSaved"));
      })}>{t("codex.plans.saveCurrent")}</Button>
    </div>
  </div>;
}

function ScenarioGallery({ view, selectedId, disabled, onSelect }: {
  view: api.CodexProjectPlansView; selectedId: string | null; disabled: boolean; onSelect: (id: string) => void;
}) {
  const { t } = useI18n();
  const headingId = useId();
  const groupName = useId();
  return <section className="asb-plan-section" aria-labelledby={headingId}>
    <h4 className="asb-group-title" id={headingId}>{t("codex.plans.savedScenarios")}</h4>
    {view.plans.length === 0 ? <p className="asb-plan-empty">{t("codex.plans.empty")}</p> : (
      <div role="radiogroup" aria-label={t("codex.plans.selectAria")} className="asb-plan-gallery">
        {view.plans.map((plan) => {
          const selected = plan.id === selectedId;
          return <label key={plan.id} className={"asb-plan-card" + (selected ? " is-selected" : "")}>
            <input type="radio" name={groupName} checked={selected} disabled={disabled} onChange={() => onSelect(plan.id)} />
            <span className="asb-plan-card-title">
              <span className="asb-plan-card-name">{plan.name}</span>
              {plan.id === view.current && <span className="asb-status-pill is-ok">
                <span className="asb-status-pill-dot" aria-hidden="true" />{t("codex.plans.currentPill")}
              </span>}
            </span>
            <SlotStrip values={scenarioSlots(view, plan, t)} />
          </label>;
        })}
      </div>
    )}
  </section>;
}

/** The actions for the selected scenario. It appears with the selection, so an
 * unselected gallery never carries empty groups or placeholder hints. */
function ScenarioPanel({ view, selected, operations, name, setName, deleting, setDeleting, setPreview, clearOutcome, reset }: {
  view: api.CodexProjectPlansView;
  selected: api.CodexProjectPlan;
  operations: CodexOperations;
  name: string;
  setName: (name: string) => void;
  deleting: boolean;
  setDeleting: (deleting: boolean) => void;
  setPreview: (preview: api.CodexProjectApplyPreview | null) => void;
  clearOutcome: () => void;
  reset: (next: api.CodexProjectPlansView) => void;
  onDelete: () => void;
}) {
  const { run, busy, changed } = operations;
  const { t } = useI18n();
  const isCurrent = selected.id === view.current;
  return <section className="asb-plan-panel" aria-label={t("codex.plans.actionsAria")}>
    <div className="asb-plan-panel-head">
      <label className="asb-field is-medium"><span>{t("codex.plans.nameLabel")}</span>
        <Input value={name} disabled={busy} onChange={(event) => setName(event.target.value)} /></label>
      <Button variant="secondary" disabled={busy || !name.trim()} onClick={() => void run(async () => {
        reset(await api.renameCodexProjectPlan(selected.id, name, view.revision));
        changed(uiMessage("codex.plans.nameSaved"));
      })}>{t("codex.plans.saveName")}</Button>
    </div>
    {deleting ? (
      <div className="asb-plan-confirm" role="group" aria-label={t("codex.plans.deleteConfirmAria")}>
        <p className="asb-warn-text">{t("codex.plans.deleteConfirm", { name: selected.name })}</p>
        <div className="asb-form-actions">
          <Button variant="secondary" disabled={busy} onClick={() => setDeleting(false)}>{t("codex.plans.keep")}</Button>
          <Button variant="danger" disabled={busy} onClick={() => void run(async () => {
            reset(await api.deleteCodexProjectPlan(selected.id, view.revision));
            onDelete();
            changed(uiMessage("codex.plans.deleted"));
          })}>{t("codex.plans.confirmDelete")}</Button>
        </div>
      </div>
    ) : (
      <div className="asb-plan-actions">
        <Button variant="primary" disabled={busy} onClick={() => void run(async () => {
          clearOutcome();
          setPreview(await api.previewCodexProjectPlanApply(selected.id, view.revision));
        })}>{t("codex.plans.previewApply")}</Button>
        <Button variant="secondary" disabled={busy} onClick={() => void run(async () => {
          reset(await api.resnapshotCodexProjectPlan(selected.id, view.revision));
          changed(uiMessage("codex.plans.updatedFromCurrent"));
        })}>{t("codex.plans.updateFromCurrent")}</Button>
        <Button variant="secondary" disabled={busy} onClick={() => void run(async () => {
          reset(await api.setCurrentCodexProjectPlan(isCurrent ? null : selected.id, view.revision));
          changed(isCurrent ? uiMessage("codex.plans.clearedCurrent") : uiMessage("codex.plans.setCurrentDone"));
        })}>{t(isCurrent ? "codex.plans.clearCurrent" : "codex.plans.setCurrentLabel")}</Button>
        <Button variant="dangerText" disabled={busy} onClick={() => setDeleting(true)}>{t("codex.plans.deleteGroup")}</Button>
      </div>
    )}
  </section>;
}

function RestorePreview({ view, preview, operations, setView, setPreview, setApplied, setWarnings }: {
  view: api.CodexProjectPlansView;
  preview: api.CodexProjectApplyPreview;
  operations: CodexOperations;
  setView: (view: api.CodexProjectPlansView) => void;
  setPreview: (preview: api.CodexProjectApplyPreview | null) => void;
  setApplied: (applied: api.CodexProjectApplyStep[]) => void;
  setWarnings: (warnings: api.CodexProjectApplyOutcome["warnings"]) => void;
}) {
  const { run, busy, changed } = operations;
  const { t } = useI18n();
  return <section className="asb-plan-panel asb-plan-preview" aria-label={t("codex.plans.previewAria")}>
    <h4 className="asb-group-title">{t("codex.plans.previewGroup")}</h4>
    <p className="asb-scope-note">{preview.autosavePlanId
      ? t("codex.plans.autosaveNote", { name: projectPlanName(view, preview.autosavePlanId) })
      : t("codex.plans.noAutosave")}</p>
    {preview.steps.length
      ? <ul className="asb-plan-steps">{preview.steps.map((step, index) => <li key={`${step.kind}-${step.target}-${index}`}>{stepText(step, t)}</li>)}</ul>
      : <p className="asb-scope-note">{t("codex.plans.uptodate")}</p>}
    {preview.warnings.map((warning) => <p key={warning.key} className="asb-warn-text">{localizedMessageText(warning, t)}</p>)}
    <div className="asb-form-actions">
      <Button variant="secondary" disabled={busy} onClick={() => setPreview(null)}>{t("codex.plans.backToScenarios")}</Button>
      <Button variant="primary" disabled={busy} onClick={() => void run(async () => {
        const outcome = await api.applyCodexProjectPlan(preview.planId, view.revision, true);
        setView(outcome.view); setPreview(null);
        setApplied(outcome.steps); setWarnings(outcome.warnings);
        changed(outcome.warnings.length
          ? uiMessage("codex.plans.appliedWithWarnings")
          : uiMessage("codex.plans.appliedClean"));
      })}>{t("codex.plans.confirmApply")}</Button>
    </div>
  </section>;
}

/** Creates, maintains, previews, and applies Codex project snapshots. */
export function ProjectPlansPane({ operations }: { operations: CodexOperations }) {
  const { run, busy } = operations;
  const { t } = useI18n();
  const [view, setView] = useState<api.CodexProjectPlansView | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [newName, setNewName] = useState("");
  const [preview, setPreview] = useState<api.CodexProjectApplyPreview | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [applied, setApplied] = useState<api.CodexProjectApplyStep[]>([]);
  const [warnings, setWarnings] = useState<api.CodexProjectApplyOutcome["warnings"]>([]);
  const reload = () => void run(async () => setView(await api.listCodexProjectPlans()));
  const clearOutcome = () => { setApplied([]); setWarnings([]); };
  const reset = (next: api.CodexProjectPlansView) => {
    setView(next); setPreview(null); clearOutcome();
  };
  // Selecting another scenario drops its pending preview, delete step and result.
  const select = (id: string) => {
    const plan = view?.plans.find((item) => item.id === id) ?? null;
    setSelectedId(plan?.id ?? null); setName(plan?.name ?? "");
    setPreview(null); setDeleting(false); clearOutcome();
  };

  useEffect(() => { void run(async () => setView(await api.listCodexProjectPlans())); }, [run]);

  if (!view) return (
    <ClientManagementModule title={t("codex.plans.title")} refreshLabel={t("codex.plans.refresh")} busy={busy} onRefresh={reload}>
      <PlanSkeleton />
    </ClientManagementModule>
  );

  const selected = view.plans.find((plan) => plan.id === selectedId) ?? null;
  return <ClientManagementModule title={t("codex.plans.title")} refreshLabel={t("codex.plans.refresh")} busy={busy} onRefresh={reload}>
    <CurrentStateStage view={view} operations={operations} newName={newName} setNewName={setNewName} reset={reset}
      onCreated={(plan) => { setSelectedId(plan?.id ?? null); setName(plan?.name ?? ""); setNewName(""); }} />
    <ScenarioGallery view={view} selectedId={selectedId} disabled={busy} onSelect={select} />
    {selected && <ScenarioPanel view={view} selected={selected} operations={operations} name={name} setName={setName}
      deleting={deleting} setDeleting={setDeleting} setPreview={setPreview} clearOutcome={clearOutcome} reset={reset}
      onDelete={() => { setSelectedId(null); setName(""); setDeleting(false); }} />}
    {preview && <RestorePreview view={view} preview={preview} operations={operations} setView={setView}
      setPreview={setPreview} setApplied={setApplied} setWarnings={setWarnings} />}
    {applied.length > 0 && <section className="asb-plan-panel asb-plan-result" aria-label={t("codex.plans.resultAria")}>
      <h4 className="asb-group-title">{t("codex.plans.resultGroup")}</h4>
      {applied.map((entry, index) => <p key={`${entry.kind}-${entry.target}-${index}`} className="asb-scope-note">{stepText(entry, t)}</p>)}
    </section>}
    {warnings.map((warning) => <p key={warning.key} className="asb-warn-text">{localizedMessageText(warning, t)}</p>)}
  </ClientManagementModule>;
}
