import type { ReactNode } from "react";
import { CUSTOM_PROBE_QUESTION_ID, type ModelUsageRange } from "../api/client";
import { Button } from "../components/Button";
import { Input } from "../components/Input";
import type { CodexProbeFormState } from "../components/CodexProbePanel";
import { RadioOption } from "../components/RadioOption";
import { Select } from "../components/Select";
import { Tabs } from "../components/Tabs";
import { Time } from "../components/Time";
import { WorkspaceHeader } from "../components/WorkspaceHeader";
import { useI18n, type TFunction } from "../i18n";
import type { MessageKey } from "../i18n/messages";
import type { useCodexOfficialReset, useCodexResetSignal } from "../components/quota-reads";
import type { useCodexProbe } from "./useCodexProbe";
import type { useModelUsageReport } from "../pages/use-model-usage-report";
import type { useResourceUsage } from "../pages/use-resource-usage";
import { USAGE_SECTIONS, type UsageSection } from "./navigation";

const RANGES: ReadonlyArray<{ value: ModelUsageRange; labelKey: MessageKey }> = [
  { value: "today", labelKey: "usage.range.today" }, { value: "last7Days", labelKey: "usage.range.last7Days" },
  { value: "last30Days", labelKey: "usage.range.last30Days" }, { value: "all", labelKey: "usage.range.all" },
];

interface RadarControlsProps {
  active: boolean;
  probe: ReturnType<typeof useCodexProbe>;
  form: CodexProbeFormState;
  onQuestionChange: (id: string | null) => void;
  runCount: number;
  onRunCountChange: (count: number) => void;
  onOpenHistory: () => void;
}

function RadarControls({ active, probe, form, onQuestionChange, runCount, onRunCountChange, onOpenHistory }: RadarControlsProps) {
  const { t } = useI18n();
  const custom = form.questionId === CUSTOM_PROBE_QUESTION_ID;
  const selected = probe.questions?.find((question) => question.id === form.questionId);
  const maxRuns = custom ? probe.customMaxRuns : selected?.maxRuns ?? null;
  const effectiveRuns = maxRuns === null ? runCount : Math.min(runCount, maxRuns);
  const ready = probe.synchronized && maxRuns !== null && probe.customTimeoutSeconds !== null
    && (!custom || (form.customQuestion.trim().length > 0
      && (form.customGradingVersion !== "1" || /^\d+$/.test(form.customAnswer.trim()))));
  const locked = !active || probe.running || probe.starting || !!probe.status?.persistPending;
  return (
    <div className="asb-model-usage-controls">
      <Select value={form.questionId} options={[
        ...(probe.questions ?? []).map((question) => ({ value: question.id, label: question.label })),
        { value: CUSTOM_PROBE_QUESTION_ID, label: t("usage.radar.customQuestion") },
      ]} placeholder={probe.questions === null ? t("usage.radar.questionsLoading") : t("usage.radar.selectQuestion")}
        ariaLabel={t("usage.radar.questionAria")} disabled={locked} onChange={onQuestionChange} />
      {maxRuns !== null && <div className="asb-run-count">
        <span className="asb-run-count-label">{t("usage.radar.runCount")}</span>
        <Input type="number" min={1} max={maxRuns} step={1} value={effectiveRuns}
          disabled={locked || maxRuns === 1} aria-label={t("usage.radar.runCountAria")}
          onChange={(event) => {
            const next = Number.parseInt(event.target.value, 10);
            if (!Number.isNaN(next)) onRunCountChange(Math.min(maxRuns, Math.max(1, next)));
          }} />
      </div>}
      {probe.running ? (
        <Button variant="secondary" disabled={probe.cancelling || probe.starting} onClick={() => void probe.cancel()}>
          {probe.cancelling ? t("usage.radar.cancelling") : t("usage.radar.cancel")}
        </Button>
      ) : (
        <Button variant="primary" disabled={locked || form.questionId === null || !ready}
          onClick={() => {
            if (form.questionId === null) return;
            void probe.start({ runCount: effectiveRuns, questionId: form.questionId,
              customQuestion: custom ? form.customQuestion.trim() : undefined,
              customAnswer: custom && form.customGradingVersion === "1" ? form.customAnswer.trim() : undefined,
              customGradingVersion: custom ? form.customGradingVersion : undefined });
          }}>
          {probe.starting ? t("usage.radar.starting") : t("usage.radar.start")}
        </Button>
      )}
      <Button variant="secondary" onClick={onOpenHistory}>{t("usage.radar.history")}</Button>
    </div>
  );
}

interface HeaderProps {
  id: string;
  section: UsageSection;
  onSectionChange: (section: UsageSection) => void;
  consumption: {
    active: boolean;
    range: ModelUsageRange;
    onRangeChange: (range: ModelUsageRange) => void;
    usage: ReturnType<typeof useModelUsageReport>;
  };
  officialReset: ReturnType<typeof useCodexOfficialReset>;
  resetSignal: ReturnType<typeof useCodexResetSignal>;
  radar: RadarControlsProps;
  resources: ReturnType<typeof useResourceUsage> & { active: boolean };
}

function ConsumptionControls({ active, range, onRangeChange, usage }: HeaderProps["consumption"]) {
  const { t } = useI18n();
  return (
    <div className="asb-model-usage-controls">
      <div className="asb-segments" role="radiogroup" aria-label={t("usage.range.aria")}>
        {RANGES.map((option) => <RadioOption key={option.value} name="model-usage-range"
          checked={range === option.value} disabled={!active} label={t(option.labelKey)}
          onChange={() => onRangeChange(option.value)} />)}
      </div>
      <Button variant="secondary" disabled={usage.loading || !active} onClick={() => void usage.refresh()}>
        {usage.loading ? t("usage.action.refreshing") : t("usage.action.refresh")}
      </Button>
    </div>
  );
}

function freshnessLabel(freshness: "cached" | "live", t: TFunction) {
  return t(freshness === "cached" ? "usage.freshness.cached" : "usage.freshness.live");
}

export function UsageWorkspaceHeader({ id, section, onSectionChange, consumption,
  officialReset, resetSignal, radar, resources }: HeaderProps) {
  const { t } = useI18n();
  const { usage } = consumption;
  const report = usage.read?.report;
  let actions: ReactNode;
  let status: ReactNode;
  if (section === "consumption") {
    actions = <ConsumptionControls {...consumption} />;
    status = report ? <>
      {t(usage.read?.freshness === "cached" ? "usage.status.snapshot" : "usage.status.current")}
      <Time iso={report.generatedAt} />
      {usage.loading ? t("usage.status.updating") : null}
    </> : t("usage.status.noSummary");
  } else if (section === "quota") {
    actions = <>
      <Button variant="secondary" disabled={officialReset.loading} onClick={() => void officialReset.readStatus()}>
        {officialReset.loading ? t("usage.quota.reading") : t("usage.quota.refreshOfficial")}
      </Button>
      <Button variant="secondary" disabled={resetSignal.loading} onClick={() => void resetSignal.readStatus()}>
        {resetSignal.loading ? t("usage.quota.reading") : t("usage.quota.refreshReset")}
      </Button>
    </>;
    status = <>
      {t("usage.quota.officialLabel")}
      {officialReset.quota ? freshnessLabel(officialReset.freshness, t) : t("usage.quota.noReadings")}
      {" · "}{t("usage.quota.resetLabel")}
      {resetSignal.snapshot ? freshnessLabel(resetSignal.snapshot.freshness, t) : t("usage.quota.noCache")}
    </>;
  } else if (section === "resources") {
    actions = <Button variant="secondary" disabled={resources.loading || !resources.active} onClick={resources.refresh}>
      {t(resources.loading ? "usage.action.refreshing" : "usage.action.refresh")}
    </Button>;
    status = resources.read ? <>
      {t("resources.sampledAt")}<Time iso={resources.read.sampledAt} />
      {resources.loading ? t("usage.status.updating") : null}
    </> : t("resources.noReadings");
  } else {
    actions = <RadarControls {...radar} />;
    status = t("usage.radar.statusNote");
  }
  return <WorkspaceHeader title={t("nav.page.usage")} primary={
    <Tabs value={section} onChange={onSectionChange} scope={id} label={t("usage.tabs.aria")}
      tabs={USAGE_SECTIONS.map((tab) => ({ value: tab.value, label: t(tab.labelKey), controls: `${id}-${tab.value}-panel` }))} />
  } primaryActions={actions} secondary={<p className="asb-header-status" role={section === "resources" ? undefined : "status"}>{status}</p>} />;
}
