import { useState } from "react";
import { RiDatabase2Line, RiLoginBoxLine, RiLogoutBoxLine, RiStackLine } from "@remixicon/react";
import {
  type ModelUsageGroup,
  type ModelUsageRange,
  type ModelUsageReport,
} from "../api/client";
import { Button } from "../components/Button";
import {
  ModelUsageDistributionChart,
} from "../components/charts/ModelUsageDistributionChart";
import { UsageTrendChart } from "../components/charts/UsageTrendChart";
import { RadioOption } from "../components/RadioOption";
import { StatCards } from "../components/application/dashboard/stat-cards";
import { Table, type TableColumn } from "../components/Table";
import { Time } from "../components/Time";
import { clientName } from "../lib/client-name";
import { TOKEN_UNIT, formatCompactTokenCount, formatTokenCount } from "../lib/token-format";
import { useModelUsageReport } from "./use-model-usage-report";

const RANGE_OPTIONS: ReadonlyArray<{ value: ModelUsageRange; label: string }> = [
  { value: "today", label: "今日" },
  { value: "last7Days", label: "近 7 天" },
  { value: "last30Days", label: "近 30 天" },
  { value: "all", label: "全部" },
];

function cachedTokenCount(group: ModelUsageGroup): number {
  return group.cacheReadInputTokens + group.cacheCreationInputTokens;
}

function reportTotal(report: ModelUsageReport, selector: (group: ModelUsageGroup) => number): number {
  return report.groups.reduce((total, group) => total + selector(group), 0);
}

function dailyTrend(report: ModelUsageReport) {
  return [
    {
      id: "fresh-input",
      label: "新输入",
      unit: TOKEN_UNIT,
      points: report.days.map((day) => ({ at: `${day.date}T12:00:00`, value: day.inputTokens })),
    },
    {
      id: "cache",
      label: "缓存",
      unit: TOKEN_UNIT,
      points: report.days.map((day) => ({
        at: `${day.date}T12:00:00`,
        value: day.cacheReadInputTokens + day.cacheCreationInputTokens,
      })),
    },
    {
      id: "output",
      label: "输出",
      unit: TOKEN_UNIT,
      points: report.days.map((day) => ({ at: `${day.date}T12:00:00`, value: day.outputTokens })),
    },
  ];
}

function modelComposition(report: ModelUsageReport) {
  return report.groups.map((group, index) => ({
    id: `${group.app}-${group.model ?? "unknown"}-${index}`,
    label: `${clientName(group.app)} · ${group.model ?? "未记录"}`,
    value: group.totalTokens,
  }));
}

const USAGE_COLUMNS: Array<TableColumn<ModelUsageGroup>> = [
  {
    key: "app",
    header: "客户端",
    render: (group) => clientName(group.app),
  },
  {
    key: "model",
    header: "模型",
    cellClassName: "asb-code",
    render: (group) => group.model ?? "未记录",
  },
  {
    key: "input",
    header: "输入（tokens）",
    cellClassName: "asb-model-usage-number",
    render: (group) => formatTokenCount(group.inputTokens),
  },
  {
    key: "cache",
    header: "缓存（tokens）",
    cellClassName: "asb-model-usage-number",
    render: (group) => formatTokenCount(cachedTokenCount(group)),
  },
  {
    key: "output",
    header: "输出（tokens）",
    cellClassName: "asb-model-usage-number",
    render: (group) => formatTokenCount(group.outputTokens),
  },
  {
    key: "total",
    header: "总计（tokens）",
    cellClassName: "asb-model-usage-number",
    render: (group) => formatTokenCount(group.totalTokens),
  },
  {
    key: "sessions",
    header: "会话",
    cellClassName: "asb-model-usage-number",
    render: (group) => formatTokenCount(group.sessionCount),
  },
];

/** Read-only local session token totals. Provider quota remains in provider panels. */
export function UsagePage({ active }: { active: boolean }) {
  const [range, setRange] = useState<ModelUsageRange>("today");
  const { read, loading, error, refresh } = useModelUsageReport(active, range);
  const report = read?.report ?? null;

  const changeRange = (nextRange: ModelUsageRange) => {
    if (nextRange === range) return;
    setRange(nextRange);
  };

  const freshInput = report ? reportTotal(report, (group) => group.inputTokens) : 0;
  const cachedInput = report ? reportTotal(report, cachedTokenCount) : 0;
  const output = report ? reportTotal(report, (group) => group.outputTokens) : 0;
  const total = report ? reportTotal(report, (group) => group.totalTokens) : 0;
  const undated = report?.unassignedTokens.totalTokens ?? 0;

  return (
    <section className="asb-panel asb-model-usage" aria-label="模型消耗">
      <div className="asb-panel-heading">
        <div>
          <h2 className="asb-panel-title">模型消耗</h2>
          <p className="asb-scope-note">
            仅汇总本地会话记录，不表示供应商剩余额度。页面可见时按本地快照时间自动刷新。
          </p>
          {report && (
            <p className="asb-model-usage-snapshot" role="status">
              {read?.freshness === "cached" ? "本地快照" : "本次汇总"}：<Time iso={report.generatedAt} />
              {loading ? " · 正在更新" : null}
            </p>
          )}
        </div>
        <div className="asb-model-usage-controls">
          <div className="asb-segments" role="radiogroup" aria-label="模型消耗时间范围">
            {RANGE_OPTIONS.map((option) => (
              <RadioOption
                key={option.value}
                name="model-usage-range"
                checked={range === option.value}
                disabled={!active}
                label={option.label}
                onChange={() => changeRange(option.value)}
              />
            ))}
          </div>
          <Button variant="secondary" disabled={loading || !active} onClick={() => void refresh()}>
            {loading ? "刷新中" : "刷新"}
          </Button>
        </div>
      </div>
      {error && <p className="asb-model-usage-notice" role="alert">{error}</p>}
      {read?.cacheWarning && <p className="asb-warn-text" role="alert">{read.cacheWarning}</p>}
      {report?.issues.length ? (
        <ul className="asb-model-usage-issues" aria-label="模型消耗提示">
          {report.issues.map((issue) => (
            <li key={`${issue.app}-${issue.message}`} className="asb-warn-text">
              {clientName(issue.app)}：{issue.message}
            </li>
          ))}
        </ul>
      ) : null}
      {loading && report === null ? (
        <p className="asb-empty asb-model-usage-empty" role="status">
          正在汇总本地会话记录…
        </p>
      ) : report?.groups.length ? (
        <>
          <div role="group" aria-label="模型消耗汇总" className="bui-scope">
            <StatCards
              variant="summary"
              stats={[
                { icon: RiStackLine, label: "总计", value: formatCompactTokenCount(total), unit: TOKEN_UNIT },
                { icon: RiLoginBoxLine, label: "新输入", value: formatCompactTokenCount(freshInput), unit: TOKEN_UNIT },
                { icon: RiDatabase2Line, label: "缓存", value: formatCompactTokenCount(cachedInput), unit: TOKEN_UNIT },
                { icon: RiLogoutBoxLine, label: "输出", value: formatCompactTokenCount(output), unit: TOKEN_UNIT },
              ]}
              columns={4}
            />
          </div>
          {undated > 0 && (
            <p className="asb-model-usage-undated" role="status">
              {formatTokenCount(undated)} tokens 未记录时间，已保留在明细总计中，但未纳入日趋势。
            </p>
          )}
          <div className="asb-model-usage-analysis">
            <section className="asb-model-usage-distribution" aria-label="模型构成">
              <ModelUsageDistributionChart
                items={modelComposition(report)}
                ariaLabel="模型消耗构成"
                emptyMessage="当前范围内没有可比较的模型消耗记录。"
              />
            </section>
            <section className="asb-model-usage-trend" aria-label="每日 Token 趋势">
              <UsageTrendChart
                series={dailyTrend(report)}
                ariaLabel="模型消耗日趋势"
                title="每日 Token 趋势"
                emptyMessage="当前范围内没有带时间的模型消耗记录。"
                sumAcrossSeries
              />
            </section>
          </div>
          <section className="asb-model-usage-detail" aria-labelledby="model-usage-detail-heading">
            <h3 id="model-usage-detail-heading">明细</h3>
            <div className="asb-model-usage-table-wrap">
              <Table
                columns={USAGE_COLUMNS}
                rows={report.groups}
                rowKey={(group, index) => `${group.app}-${group.model ?? "unknown"}-${index}`}
                ariaLabel="模型消耗"
                className="asb-model-usage-table"
              />
            </div>
          </section>
        </>
      ) : report ? (
        <p className="asb-empty asb-model-usage-empty">当前范围内没有可用的模型消耗记录。</p>
      ) : null}
    </section>
  );
}
