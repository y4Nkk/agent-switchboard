import { useCallback, useEffect, useRef, useState } from "react";
import {
  getModelUsageReport,
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
import { Table, type TableColumn } from "../components/Table";
import { clientName } from "../lib/client-name";

const RANGE_OPTIONS: ReadonlyArray<{ value: ModelUsageRange; label: string }> = [
  { value: "today", label: "今日" },
  { value: "last7Days", label: "近 7 天" },
  { value: "last30Days", label: "近 30 天" },
  { value: "all", label: "全部" },
];

const tokenFormatter = new Intl.NumberFormat("zh-CN");

function tokenCount(value: number): string {
  return tokenFormatter.format(value);
}

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
      unit: "token",
      points: report.days.map((day) => ({ at: `${day.date}T12:00:00`, value: day.inputTokens })),
    },
    {
      id: "cache",
      label: "缓存",
      unit: "token",
      points: report.days.map((day) => ({
        at: `${day.date}T12:00:00`,
        value: day.cacheReadInputTokens + day.cacheCreationInputTokens,
      })),
    },
    {
      id: "output",
      label: "输出",
      unit: "token",
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
    header: "输入",
    cellClassName: "asb-model-usage-number",
    render: (group) => tokenCount(group.inputTokens),
  },
  {
    key: "cache",
    header: "缓存",
    cellClassName: "asb-model-usage-number",
    render: (group) => tokenCount(cachedTokenCount(group)),
  },
  {
    key: "output",
    header: "输出",
    cellClassName: "asb-model-usage-number",
    render: (group) => tokenCount(group.outputTokens),
  },
  {
    key: "total",
    header: "总计",
    cellClassName: "asb-model-usage-number",
    render: (group) => tokenCount(group.totalTokens),
  },
  {
    key: "sessions",
    header: "会话",
    cellClassName: "asb-model-usage-number",
    render: (group) => tokenCount(group.sessionCount),
  },
];

function errorMessage(caught: unknown): string {
  return (caught as { message?: string }).message ?? "无法汇总本地模型消耗";
}

/** Read-only local session token totals. Provider quota remains in provider panels. */
export function UsagePage({ active }: { active: boolean }) {
  const [range, setRange] = useState<ModelUsageRange>("today");
  const [report, setReport] = useState<ModelUsageReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestVersion = useRef(0);

  const refresh = useCallback(async () => {
    if (!active) return;

    const version = ++requestVersion.current;
    setLoading(true);
    setError(null);
    try {
      const next = await getModelUsageReport(range);
      if (requestVersion.current === version) setReport(next);
    } catch (caught) {
      if (requestVersion.current === version) setError(errorMessage(caught));
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }, [active, range]);

  useEffect(() => {
    if (!active) {
      requestVersion.current += 1;
      setLoading(false);
      return;
    }
    void refresh();
  }, [active, refresh]);

  const changeRange = (nextRange: ModelUsageRange) => {
    if (nextRange === range) return;
    setRange(nextRange);
    setReport(null);
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
          <p className="asb-scope-note">仅汇总本地会话记录，不表示供应商剩余额度。</p>
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
          <dl className="asb-model-usage-summary" aria-label="模型消耗汇总">
            <div>
              <dt>总计</dt>
              <dd>{tokenCount(total)}</dd>
            </div>
            <div>
              <dt>新输入</dt>
              <dd>{tokenCount(freshInput)}</dd>
            </div>
            <div>
              <dt>缓存</dt>
              <dd>{tokenCount(cachedInput)}</dd>
            </div>
            <div>
              <dt>输出</dt>
              <dd>{tokenCount(output)}</dd>
            </div>
          </dl>
          {undated > 0 && (
            <p className="asb-model-usage-undated" role="status">
              {tokenCount(undated)} token 未记录时间，已保留在明细总计中，但未纳入日趋势。
            </p>
          )}
          <div className="asb-model-usage-analysis">
            <section className="asb-model-usage-trend" aria-labelledby="model-usage-trend-heading">
              <h3 id="model-usage-trend-heading">日趋势</h3>
              <UsageTrendChart
                series={dailyTrend(report)}
                ariaLabel="模型消耗日趋势"
                emptyMessage="当前范围内没有带时间的模型消耗记录。"
              />
            </section>
            <section className="asb-model-usage-distribution" aria-labelledby="model-usage-distribution-heading">
              <h3 id="model-usage-distribution-heading">模型构成</h3>
              <ModelUsageDistributionChart
                items={modelComposition(report)}
                ariaLabel="模型消耗构成"
                emptyMessage="当前范围内没有可比较的模型消耗记录。"
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
