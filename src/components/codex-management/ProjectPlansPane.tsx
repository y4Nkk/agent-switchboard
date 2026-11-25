import { useEffect, useId, useRef, useState, type ComponentType, type Ref } from "react";
import * as api from "../../api/codex-project-plans";
import type { MessageKey, TFunction } from "../../i18n";
import { useI18n } from "../../i18n";
import { localizedMessageText, uiMessage } from "../../i18n/errors";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { ConfirmSheet } from "../ConfirmSheet";
import { FactList } from "../FactList";
import { Input } from "../Input";
import { ClientManagementModule } from "../client-management/ClientManagementModule";
import { useClientToolDialog } from "../client-management/use-client-tool-dialog";
import { McpKindIcon, ProviderKindIcon, ScrollTextIcon, SkillKindIcon } from "../icons";
import type { CodexOperations } from "./operations";

type ScenarioDialog =
  | { kind: "restore"; view: api.CodexProjectPlansView; preview: api.CodexProjectApplyPreview }
  | { kind: "delete"; plan: api.CodexProjectPlan; revision: string };

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

function ScenarioGallery({ view, selectedId, disabled, headingRef, onSelect }: {
  view: api.CodexProjectPlansView; selectedId: string | null; disabled: boolean;
  headingRef: Ref<HTMLHeadingElement>; onSelect: (id: string) => void;
}) {
  const { t } = useI18n();
  const headingId = useId();
  const groupName = useId();
  return <section className="asb-plan-section" aria-labelledby={headingId}>
    <h4 className="asb-group-title" id={headingId} ref={headingRef} tabIndex={-1}>{t("codex.plans.savedScenarios")}</h4>
    {view.plans.length === 0 ? <p className="asb-plan-empty">{t("codex.plans.empty")}</p> : (
      <div role="radiogroup" aria-label={t("codex.plans.selectAria")} className="asb-plan-gallery">
        {view.plans.map((plan) => (
          <div key={plan.id} className="asb-plan-card">
            <label className="asb-plan-card-title">
              <input type="radio" name={groupName} checked={plan.id === selectedId} disabled={disabled}
                onChange={() => onSelect(plan.id)} />
              <span className="asb-plan-card-name">{plan.name}</span>
              {plan.id === view.current && <span className="asb-status-pill is-ok">
                <span className="asb-status-pill-dot" aria-hidden="true" />{t("codex.plans.currentPill")}
              </span>}
            </label>
            <SlotStrip values={scenarioSlots(view, plan, t)} />
          </div>
        ))}
      </div>
    )}
  </section>;
}

/** The actions for the selected scenario. It appears with the selection, so an
 * unselected gallery never carries empty groups or placeholder hints. The
 * delete confirmation and the restore preview are the pane's dialogs. */
function ScenarioPanel({ view, selected, operations, name, setName, onPreview, reset, onDelete }: {
  view: api.CodexProjectPlansView;
  selected: api.CodexProjectPlan;
  operations: CodexOperations;
  name: string;
  setName: (name: string) => void;
  /** Asks the pane to preview restoring the selected scenario. */
  onPreview: (action: HTMLButtonElement) => void;
  reset: (next: api.CodexProjectPlansView) => void;
  /** Asks the pane to confirm deleting the selected scenario. */
  onDelete: (action: HTMLButtonElement) => void;
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
    <div className="asb-plan-actions">
      <Button variant="primary" disabled={busy} onClick={(event) => onPreview(event.currentTarget)}>{t("codex.plans.previewApply")}</Button>
      <Button variant="secondary" disabled={busy} onClick={() => void run(async () => {
        reset(await api.resnapshotCodexProjectPlan(selected.id, view.revision));
        changed(uiMessage("codex.plans.updatedFromCurrent"));
      })}>{t("codex.plans.updateFromCurrent")}</Button>
      <Button variant="secondary" disabled={busy} onClick={() => void run(async () => {
        reset(await api.setCurrentCodexProjectPlan(isCurrent ? null : selected.id, view.revision));
        changed(isCurrent ? uiMessage("codex.plans.clearedCurrent") : uiMessage("codex.plans.setCurrentDone"));
      })}>{t(isCurrent ? "codex.plans.clearCurrent" : "codex.plans.setCurrentLabel")}</Button>
      <Button variant="dangerText" disabled={busy} onClick={(event) => onDelete(event.currentTarget)}>{t("codex.plans.deleteGroup")}</Button>
    </div>
  </section>;
}

/** The restore plan as a dialog: what will be written and what is saved first,
 * then the one confirmation that applies it. A failed apply stays here with its
 * reason; a successful one is reported on the page. */
function RestorePreview({ view, preview, operations, onClose, onApplied }: {
  view: api.CodexProjectPlansView;
  preview: api.CodexProjectApplyPreview;
  operations: CodexOperations;
  onClose: () => void;
  onApplied: (outcome: api.CodexProjectApplyOutcome) => void;
}) {
  const { run, busy, error } = operations;
  const { t } = useI18n();
  return <AppDialog title={t("codex.plans.previewGroup")} busy={busy} onClose={onClose} footer={<>
    <Button autoFocus variant="secondary" disabled={busy} onClick={onClose}>{t("codex.plans.backToScenarios")}</Button>
    <Button variant="primary" disabled={busy} onClick={() => void run(async () => {
      onApplied(await api.applyCodexProjectPlan(preview.planId, view.revision, true));
    })}>{t("codex.plans.confirmApply")}</Button>
  </>}>
    {error && <p role="alert" className="asb-field-error">{error}</p>}
    <p className="asb-scope-note">{preview.autosavePlanId
      ? t("codex.plans.autosaveNote", { name: projectPlanName(view, preview.autosavePlanId) })
      : t("codex.plans.noAutosave")}</p>
    {preview.steps.length
      ? <ul className="asb-plan-steps">{preview.steps.map((step, index) => <li key={`${step.kind}-${step.target}-${index}`}>{stepText(step, t)}</li>)}</ul>
      : <p className="asb-scope-note">{t("codex.plans.uptodate")}</p>}
    {preview.warnings.map((warning) => <p key={warning.key} className="asb-warn-text">{localizedMessageText(warning, t)}</p>)}
  </AppDialog>;
}

function DeleteScenarioConfirmation({ plan, revision, operations, onCancel, onDeleted }: {
  plan: api.CodexProjectPlan;
  revision: api.CodexProjectPlansView["revision"];
  operations: CodexOperations;
  onCancel: () => void;
  onDeleted: (view: api.CodexProjectPlansView) => void;
}) {
  const { busy, error, run } = operations;
  const { t } = useI18n();
  return <ConfirmSheet title={t("codex.plans.deleteTitle")} busy={busy} destructive
    confirmLabel={t("codex.plans.confirmDelete")}
    onConfirm={() => void run(async () => onDeleted(await api.deleteCodexProjectPlan(plan.id, revision)))}
    onCancel={onCancel}>
    <ul className="asb-dialog-details">
      <li>{t("codex.plans.deleteLine1", { name: plan.name })}</li>
      <li>{t("codex.plans.deleteLine2")}</li>
    </ul>
    {error && <p role="alert" className="asb-field-error">{error}</p>}
  </ConfirmSheet>;
}

/** Creates, maintains, previews, and applies Codex project snapshots. Its two
 * dialogs close with the pane: one left in a hidden page keeps the app inert. */
export function ProjectPlansPane({ operations, active }: { operations: CodexOperations; active: boolean }) {
  const { run, busy, changed, setError } = operations;
  const { t } = useI18n();
  const [view, setView] = useState<api.CodexProjectPlansView | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [newName, setNewName] = useState("");
  const [outcome, setOutcome] = useState<Pick<api.CodexProjectApplyOutcome, "steps" | "warnings"> | null>(null);
  const galleryHeading = useRef<HTMLHeadingElement>(null);
  const { dialog, open, load, close } = useClientToolDialog<ScenarioDialog>(active, busy);
  const reload = () => void run(async () => setView(await api.listCodexProjectPlans()));
  const reset = (next: api.CodexProjectPlansView) => {
    setView(next); setOutcome(null);
  };
  const select = (id: string) => {
    const plan = view?.plans.find((item) => item.id === id) ?? null;
    setSelectedId(plan?.id ?? null); setName(plan?.name ?? "");
    setOutcome(null);
  };

  useEffect(() => { void run(async () => setView(await api.listCodexProjectPlans())); }, [run]);

  if (!view) return <ClientManagementModule title={t("codex.plans.title")} refreshLabel={t("codex.plans.refresh")} busy={busy} onRefresh={reload}>
    {busy && <PlanSkeleton />}
  </ClientManagementModule>;

  const selected = view.plans.find((plan) => plan.id === selectedId) ?? null;
  const openPreview = (action: HTMLButtonElement, planId: string) => void load(async () => {
    setOutcome(null);
    return { kind: "restore", view, preview: await api.previewCodexProjectPlanApply(planId, view.revision) };
  }, run, () => action);
  return <ClientManagementModule title={t("codex.plans.title")} refreshLabel={t("codex.plans.refresh")} busy={busy} onRefresh={reload}>
    <CurrentStateStage view={view} operations={operations} newName={newName} setNewName={setNewName} reset={reset}
      onCreated={(plan) => { setSelectedId(plan?.id ?? null); setName(plan?.name ?? ""); setNewName(""); }} />
    <ScenarioGallery view={view} selectedId={selectedId} disabled={busy} headingRef={galleryHeading} onSelect={select} />
    {selected && <ScenarioPanel view={view} selected={selected} operations={operations} name={name} setName={setName}
      onPreview={(action) => openPreview(action, selected.id)} reset={reset}
      onDelete={(action) => {
        setError(null);
        open({ kind: "delete", plan: selected, revision: view.revision },
          () => action.isConnected ? action : galleryHeading.current);
      }} />}
    {outcome && outcome.steps.length > 0 && <section className="asb-plan-panel" aria-label={t("codex.plans.resultAria")}>
      <h4 className="asb-group-title">{t("codex.plans.resultGroup")}</h4>
      {outcome.steps.map((entry, index) => <p key={`${entry.kind}-${entry.target}-${index}`} className="asb-scope-note">{stepText(entry, t)}</p>)}
    </section>}
    {outcome?.warnings.map((warning) => <p key={warning.key} className="asb-warn-text">{localizedMessageText(warning, t)}</p>)}
    {dialog?.kind === "restore" && <RestorePreview view={dialog.view} preview={dialog.preview} operations={operations}
      onClose={close}
      onApplied={({ view: next, steps, warnings }) => {
        setView(next); close();
        setOutcome({ steps, warnings });
        changed(warnings.length
          ? uiMessage("codex.plans.appliedWithWarnings")
          : uiMessage("codex.plans.appliedClean"));
      }} />}
    {dialog?.kind === "delete" && <DeleteScenarioConfirmation plan={dialog.plan} revision={dialog.revision}
      operations={operations} onCancel={close}
      onDeleted={(next) => {
        reset(next);
        setSelectedId(null); setName(""); close();
        changed(uiMessage("codex.plans.deleted"));
      }} />}
  </ClientManagementModule>;
}
