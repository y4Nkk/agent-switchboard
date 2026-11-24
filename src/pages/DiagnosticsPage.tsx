import { useEffect, useState } from "react";
import type { RuntimeLogLevel } from "../api/client";
import { DIAGNOSTIC_SECTIONS, type DiagnosticSection } from "../app/navigation";
import { ConfigStatusPanel, type ConfigStatusPanelProps } from "../components/ConfigStatusPanel";
import { Button } from "../components/Button";
import { RadioOption } from "../components/RadioOption";
import { Select } from "../components/Select";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { RuntimeOverviewPanel } from "../components/RuntimeOverviewPanel";
import { PageErrorBoundary } from "../components/PageErrorBoundary";
import { Tabs } from "../components/Tabs";
import { useI18n } from "../i18n";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { LogsPage } from "./LogsPage";
import { LEVEL_FILTERS, LOG_LEVEL_OPTIONS } from "../lib/runtime-log-labels";
import { useRuntimeLogs } from "./use-runtime-logs";

interface DiagnosticsPageProps extends ConfigStatusPanelProps {
  active: boolean;
  leaving: boolean;
  section: DiagnosticSection;
  onSectionChange: (section: DiagnosticSection) => void;
  onRefresh: () => void;
  logLevel: RuntimeLogLevel | null;
  onLogLevelChange: (level: RuntimeLogLevel) => void;
}

function LogControls({ logs, logLevel, onLogLevelChange, busy }: Pick<DiagnosticsPageProps,
  "logLevel" | "onLogLevelChange" | "busy"> & { logs: ReturnType<typeof useRuntimeLogs> }) {
  const { t } = useI18n();
  return <>
    <div className="asb-runtime-log-level-control">
      <span className="asb-runtime-log-level-label">{t("backup.diagnostics.recordLevel")}</span>
      <Select value={logLevel}
        options={LOG_LEVEL_OPTIONS.map((option) => ({ value: option.value, label: t(option.labelKey) }))}
        ariaLabel={t("backup.diagnostics.recordLevel")} placeholder={t("backup.diagnostics.loading")}
        disabled={busy || logLevel === null} onChange={(level) => onLogLevelChange(level as RuntimeLogLevel)} />
    </div>
    <div className="asb-segments" role="radiogroup" aria-label={t("backup.diagnostics.levelFilterAria")}>
      {LEVEL_FILTERS.map((option) => <RadioOption key={option.value} name="runtime-log-level-filter"
        checked={logs.filter === option.value} disabled={false} label={t(option.labelKey)} onChange={() => logs.setFilter(option.value)} />)}
    </div>
    <Button variant="secondary" disabled={logs.loading} onClick={() => void logs.refresh()}>
      {logs.loading ? t("backup.diagnostics.refreshing") : t("backup.diagnostics.refresh")}
    </Button>
    <Button variant="secondary" disabled={logs.openingFolder} onClick={() => void logs.openLogDirectory()}>
      {logs.openingFolder ? t("backup.diagnostics.opening") : t("backup.diagnostics.openFolder")}
    </Button>
  </>;
}

export function DiagnosticsPage(props: DiagnosticsPageProps) {
  const { active, leaving, section, onSectionChange, profiles, busy } = props;
  const { t } = useI18n();
  const visible = useWorkspaceVisible(active);
  const [visited, setVisited] = useState<DiagnosticSection[]>([]);
  useEffect(() => {
    if (active) setVisited((current) => current.includes(section) ? current : [...current, section]);
  }, [active, section]);
  const opened = (target: DiagnosticSection) => visited.includes(target) || ((active || leaving) && section === target);
  const logsOpened = opened("logs");
  const logs = useRuntimeLogs(visible && section === "logs", leaving);
  return (
    <section className="asb-diagnostics" aria-label={t("backup.diagnostics.aria")}>
      {/* 设置内容区的子页：模块级 h3 标题独占第一行；第二行是分类页签与
      页签级动作，视图控制行只挂当前页签自己的控制项。 */}
      <ModuleHeader
        title={t("backup.diagnostics.title")}
        primary={
          <Tabs value={section} onChange={onSectionChange} scope="diagnostics" label={t("backup.diagnostics.tabsAria")}
            tabs={DIAGNOSTIC_SECTIONS.map((tab) => ({ value: tab.value, label: t(tab.labelKey), controls: `diagnostics-${tab.value}-panel` }))} />
        }
        primaryActions={section === "configuration" ? (
          <Button variant="secondary" disabled={busy} onClick={props.onRefresh}>{t("backup.diagnostics.refreshStatus")}</Button>
        ) : undefined}
        secondary={section === "logs" ? (
          <LogControls logs={logs} logLevel={props.logLevel} onLogLevelChange={props.onLogLevelChange} busy={busy} />
        ) : undefined}
      />
      <div id="diagnostics-configuration-panel" role="tabpanel" aria-labelledby="diagnostics-configuration-tab" hidden={section !== "configuration"}>
        <PageErrorBoundary active={active && section === "configuration"}>
          {opened("configuration") && <div className="asb-diagnostics-configuration">
            <ConfigStatusPanel statuses={props.statuses} profiles={profiles} locks={props.locks}
              busy={busy} onRecoverLock={props.onRecoverLock} />
            <RuntimeOverviewPanel />
          </div>}
        </PageErrorBoundary>
      </div>
      <div id="diagnostics-logs-panel" role="tabpanel" aria-labelledby="diagnostics-logs-tab" hidden={section !== "logs"}>
        <PageErrorBoundary active={active && section === "logs"}>
          {logsOpened && <LogsPage logs={logs} />}
        </PageErrorBoundary>
      </div>
    </section>
  );
}
