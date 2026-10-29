import { useEffect, useId, useMemo, useState } from "react";
import { CUSTOM_PROBE_QUESTION_ID, type CodexProbeGradingVersion, type CodexProbeQuestionSnapshot,
  type ModelUsageRange, type ProviderRecord, type CodexProviderRecord } from "../api/client";
import { ProviderQuotaOverview } from "../components/ProviderQuotaOverview";
import { quotaProviders } from "../lib/quota-profiles";
import { useProviderQuotaRefresh } from "../pages/use-provider-quota-refresh";
import { CodexOfficialResetPanel } from "../components/CodexOfficialResetPanel";
import { CodexProbeHistory } from "../components/CodexProbeHistory";
import { CodexProbePanel, type CodexProbeFormState } from "../components/CodexProbePanel";
import { CodexResetPanel } from "../components/CodexResetPanel";
import { useCodexOfficialReset, useCodexResetSignal } from "../components/quota-reads";
import type { useCodexProbe } from "./useCodexProbe";
import { useI18n } from "../i18n";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { UsagePage } from "../pages/UsagePage";
import { ResourceUsagePage } from "../pages/ResourceUsagePage";
import { useResourceUsage } from "../pages/use-resource-usage";
import { useModelUsageReport } from "../pages/use-model-usage-report";
import { UsageWorkspaceHeader } from "./UsageWorkspaceHeader";
import type { UsageSection } from "./navigation";

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
  const [range, setRange] = useState<ModelUsageRange>("last7Days");
  useEffect(() => { if (section === "radar") setRadarOpened(true); }, [section]);
  const consumptionActive = visible && section === "consumption";
  const usage = useModelUsageReport(consumptionActive, range);
  const resourcesActive = visible && section === "resources";
  const resources = useResourceUsage(resourcesActive);
  const showResources = section === "resources" || resources.read !== null || resources.error !== null;
  const radarActive = visible && section === "radar";
  const { form, runCount, setRunCount, setQuestionId, reuseQuestion } = useProbeForm(probe, () => setHistoryOpen(false));
  const quotaActive = visible && section === "quota";
  const quotaProfiles = useMemo(() => quotaProviders(claudeRecords, codexRecords), [claudeRecords, codexRecords]);
  const providerQuota = useProviderQuotaRefresh(quotaActive, quotaProfiles);
  const showHistory = section === "radar" && historyOpen;
  const officialReset = useCodexOfficialReset(quotaActive);
  const resetSignal = useCodexResetSignal(quotaActive);
  return (
    <section className="asb-page-stack" aria-label={t("usage.workspace.aria")}>
      <div hidden={showHistory}>
        <UsageWorkspaceHeader id={id} section={section} onSectionChange={onSectionChange}
          consumption={{ active: consumptionActive, range, onRangeChange: setRange, usage }}
          resources={{ ...resources, active: resourcesActive }}
          officialReset={officialReset} resetSignal={resetSignal} providerQuota={providerQuota}
          radar={{ active: radarActive, probe, form, onQuestionChange: setQuestionId,
            runCount, onRunCountChange: setRunCount,
            onOpenHistory: () => { setHistoryOpened(true); setHistoryOpen(true); } }} />
        <div id={`${id}-consumption-panel`} role="tabpanel" aria-labelledby={`${id}-consumption-tab`} hidden={section !== "consumption"}>
          {consumptionActive && <UsagePage usage={usage} />}
        </div>
        <div id={`${id}-quota-panel`} role="tabpanel" aria-labelledby={`${id}-quota-tab`} hidden={section !== "quota"}>
          {quotaActive && <div className="asb-page-stack">
            <ProviderQuotaOverview profiles={quotaProfiles} />
            <CodexOfficialResetPanel read={officialReset} />
            <CodexResetPanel read={resetSignal} />
          </div>}
        </div>
        <div id={`${id}-radar-panel`} role="tabpanel" aria-labelledby={`${id}-radar-tab`} hidden={section !== "radar"}>
          {radarOpened && <CodexProbePanel probe={probe} form={form} />}
        </div>
        <div id={`${id}-resources-panel`} role="tabpanel" aria-labelledby={`${id}-resources-tab`} hidden={section !== "resources"}>
          {showResources && <ResourceUsagePage resources={resources} />}
        </div>
      </div>
      {historyOpened && (
        <div className="asb-editor-route" hidden={!showHistory}>
          <CodexProbeHistory active={radarActive && showHistory} probe={probe} onClose={() => setHistoryOpen(false)}
            onReuseQuestion={reuseQuestion} onChanged={probe.refresh} />
        </div>
      )}
    </section>
  );
}
