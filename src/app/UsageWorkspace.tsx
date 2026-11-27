import { useEffect, useId, useMemo, useState, type ReactNode } from "react";
import { PageErrorBoundary } from "../components/PageErrorBoundary";
import { CUSTOM_PROBE_QUESTION_ID, type CodexProbeGradingVersion, type CodexProbeQuestionSnapshot,
  type ModelUsageRange, type ProviderRecord, type CodexProviderRecord } from "../api/client";
import { ProviderQuotaOverview } from "../components/ProviderQuotaOverview";
import { quotaProviders } from "../lib/quota-profiles";
import { useProviderQuotaRefresh } from "../pages/use-provider-quota-refresh";
import { useProviderQuotaRead } from "../pages/use-provider-quota-read";
import { CodexOfficialResetPanel } from "../components/CodexOfficialResetPanel";
import { CodexProbeHistory } from "../components/CodexProbeHistory";
import { CodexProbePanel } from "../components/CodexProbePanel";
import type { CodexProbeFormState } from "../components/CodexProbeSetup";
import { CodexResetPanel } from "../components/CodexResetPanel";
import { useCodexOfficialReset, useCodexResetSignal } from "../components/quota-reads";
import type { useCodexProbe } from "./useCodexProbe";
import { useI18n } from "../i18n";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { UsagePage } from "../pages/UsagePage";
import { RequestUsagePanel } from "../components/request-usage/RequestUsagePanel";
import { ResourceUsagePage } from "../pages/ResourceUsagePage";
import { useResourceUsage } from "../pages/use-resource-usage";
import { useModelUsageReport } from "../pages/use-model-usage-report";
import { useRequestUsage } from "../pages/use-request-usage";
import { UsageWorkspaceHeader } from "./UsageWorkspaceHeader";
import { useOnReturnToRoot } from "./PageRoot";
import { useUsageOpenTiming } from "./usage-open-timing";
import type { UsageSection } from "./navigation";

function UsagePanel({ id, section, selected, active, children }: {
  id: string; section: UsageSection; selected: UsageSection; active: boolean; children: ReactNode;
}) {
  return <div id={`${id}-${section}-panel`} role="tabpanel" aria-labelledby={`${id}-${section}-tab`} hidden={section !== selected}>
    <PageErrorBoundary active={active && section === selected}>{children}</PageErrorBoundary>
  </div>;
}

function useProbeForm(probe: ReturnType<typeof useCodexProbe>, closeHistory: () => void) {
  const [questionId, setQuestionId] = useState<string | null>(null);
  const [runCount, setRunCount] = useState(3);
  const [customQuestion, setCustomQuestion] = useState("");
  const [customAnswer, setCustomAnswer] = useState("");
  const [customGradingVersion, setCustomGradingVersion] = useState<CodexProbeGradingVersion>("1");
  useEffect(() => {
    if (questionId === null && probe.questions && probe.questions.length > 0) {
      setQuestionId(probe.questions[0].id);
    }
  }, [questionId, probe.questions]);
  const form: CodexProbeFormState = { questionId, customQuestion, customAnswer, customGradingVersion,
    onCustomQuestionChange: setCustomQuestion, onCustomAnswerChange: setCustomAnswer,
    onCustomGradingChange: setCustomGradingVersion };
  const reuseQuestion = (question: CodexProbeQuestionSnapshot,
    plannedRuns: number, gradingVersion: CodexProbeGradingVersion) => {
    setQuestionId(CUSTOM_PROBE_QUESTION_ID);
    setCustomQuestion(question.text);
    setCustomAnswer(question.expectedAnswer);
    setCustomGradingVersion(gradingVersion);
    setRunCount(plannedRuns);
    closeHistory();
  };
  return { form, runCount, setRunCount, setQuestionId, reuseQuestion };
}

export function UsageWorkspace({ active, section, onSectionChange, probe, claudeRecords, codexRecords }: {
  active: boolean;
  section: UsageSection;
  onSectionChange: (section: UsageSection) => void;
  probe: ReturnType<typeof useCodexProbe>;
  claudeRecords: ProviderRecord[];
  codexRecords: CodexProviderRecord[];
}) {
  const { t } = useI18n();
  const id = useId();
  const visible = useWorkspaceVisible(active);
  const [radarOpened, setRadarOpened] = useState(section === "radar");
  const [historyOpened, setHistoryOpened] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const showHistory = section === "radar" && historyOpen;
  useOnReturnToRoot(active && showHistory, () => setHistoryOpen(false));
  const [range, setRange] = useState<ModelUsageRange>("last7Days");
  useEffect(() => { if (section === "radar") setRadarOpened(true); }, [section]);
  const consumptionActive = visible && section === "consumption";
  const usage = useModelUsageReport(consumptionActive, range);
  const performance = useRequestUsage(visible && section === "requestPerformance");
  const resourcesActive = visible && section === "resources";
  const resources = useResourceUsage(resourcesActive);
  const showResources = section === "resources" || resources.read !== null || resources.error !== null;
  const radarActive = visible && section === "radar";
  const { form, runCount, setRunCount, setQuestionId, reuseQuestion } = useProbeForm(probe, () => setHistoryOpen(false));
  const quotaActive = visible && section === "quota";
  const quotaProfiles = useMemo(() => quotaProviders(claudeRecords, codexRecords), [claudeRecords, codexRecords]);
  const providerQuota = useProviderQuotaRefresh(quotaActive, quotaProfiles);
  const quotaRead = useProviderQuotaRead(quotaActive, quotaProfiles);
  const officialReset = useCodexOfficialReset(quotaActive);
  const resetSignal = useCodexResetSignal(quotaActive);
  const hasData: Record<UsageSection, boolean> = {
    consumption: usage.read !== null,
    requestPerformance: performance.report !== null,
    quota: quotaRead.ready && (!officialReset.cacheLoading || !resetSignal.runway.cacheLoading || !resetSignal.hub.cacheLoading),
    resources: resources.read !== null,
    radar: probe.questions !== null && probe.synchronized,
  };
  useUsageOpenTiming(visible ? section : null, hasData[section]);
  return (
    <section className="asb-page-stack" aria-label={t("usage.workspace.aria")}>
      <div hidden={showHistory}>
        <UsageWorkspaceHeader id={id} section={section} onSectionChange={onSectionChange}
          usage={usage} consumptionActive={consumptionActive}
          onOpenHistory={() => { setHistoryOpened(true); setHistoryOpen(true); }} />
        <UsagePanel id={id} section="consumption" selected={section} active={active}>
          {consumptionActive && <UsagePage usage={usage} range={range} onRangeChange={setRange} />}
        </UsagePanel>
        <UsagePanel id={id} section="requestPerformance" selected={section} active={active}>
          {performance.active && <RequestUsagePanel performance={performance} />}
        </UsagePanel>
        <UsagePanel id={id} section="quota" selected={section} active={active}>
          {quotaActive && <div className="asb-usage-stack">
            <CodexResetPanel read={resetSignal} />
            <ProviderQuotaOverview profiles={quotaProfiles} quota={quotaRead} refresh={providerQuota} />
            <CodexOfficialResetPanel read={officialReset} />
          </div>}
        </UsagePanel>
        <UsagePanel id={id} section="radar" selected={section} active={active && !showHistory}>
          {(section === "radar" || radarOpened) && <CodexProbePanel probe={probe} form={form}
            active={radarActive && !showHistory} runCount={runCount} onRunCountChange={setRunCount} onQuestionChange={setQuestionId} />}
        </UsagePanel>
        <UsagePanel id={id} section="resources" selected={section} active={active}>
          {showResources && <ResourceUsagePage resources={resources} active={resourcesActive} />}
        </UsagePanel>
      </div>
      {historyOpened && (
        <div className="asb-subpage-route" hidden={!showHistory}>
          <PageErrorBoundary active={active && showHistory}>
            <CodexProbeHistory open={showHistory} active={radarActive && showHistory} probe={probe} onClose={() => setHistoryOpen(false)}
              onReuseQuestion={reuseQuestion} onChanged={probe.refresh} />
          </PageErrorBoundary>
        </div>
      )}
    </section>
  );
}
