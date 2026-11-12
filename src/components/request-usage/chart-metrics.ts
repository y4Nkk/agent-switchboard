import type { MeasurementScope, RequestUsageMetrics } from "../../api/client";
import type { TFunction } from "../../i18n";
import type { MessageKey } from "../../i18n/messages";
import type { UsageTrendValueKind } from "../charts/chart-data";
import { cost, duration, number, percent, speed, tokens } from "./format";

export type RequestUsageTrend = "speedP50" | "firstTokenP50Ms" | "durationP50Ms" | "failureRate";
export type RequestUsageGroupMetric = "records" | "outputRateP50" | "firstTokenP50Ms"
  | "durationP50Ms" | "successRatio" | "cacheHitRate" | "estimatedCostUsd";
export type RequestUsageRecordMetric = "durationMs" | "firstTokenMs" | "firstContentMs"
  | "outputTokensPerSecond" | "inputTokens" | "outputTokens";

interface MetricChoice {
  labelKey: MessageKey;
  generationOnly?: boolean;
}
type ValueFormatter = (value: number | null, language: string) => string;
interface GroupMetric extends MetricChoice {
  format: ValueFormatter;
  samples?: (metrics: RequestUsageMetrics) => number;
  companion?: { key: "outputRateP10" | "firstTokenP95Ms" | "durationP95Ms"; label: "P10" | "P95" };
  maximum?: number;
}

export const TREND_METRICS: Record<RequestUsageTrend, MetricChoice & {
  unit: string; scale: number; kind: UsageTrendValueKind;
}> = {
  speedP50: { labelKey: "usagePerformance.speed", unit: "tok/s", scale: 1, kind: "generic", generationOnly: true },
  firstTokenP50Ms: { labelKey: "usagePerformance.firstToken", unit: "s", scale: 0.001, kind: "generic", generationOnly: true },
  durationP50Ms: { labelKey: "usagePerformance.durationP50", unit: "s", scale: 0.001, kind: "generic" },
  failureRate: { labelKey: "usagePerformance.trend.failures", unit: "%", scale: 1, kind: "percentage" },
};

export const GROUP_METRICS: Record<RequestUsageGroupMetric, GroupMetric> = {
  records: { labelKey: "usagePerformance.recordsCount", format: number },
  outputRateP50: {
    labelKey: "usagePerformance.speed", format: speed, generationOnly: true, samples: (metrics) => metrics.speedSamples,
    companion: { key: "outputRateP10", label: "P10" },
  },
  firstTokenP50Ms: {
    labelKey: "usagePerformance.firstToken", format: duration, generationOnly: true, samples: (metrics) => metrics.firstTokenSamples,
    companion: { key: "firstTokenP95Ms", label: "P95" },
  },
  durationP50Ms: {
    labelKey: "usagePerformance.durationP50", format: duration, samples: (metrics) => metrics.durationSamples,
    companion: { key: "durationP95Ms", label: "P95" },
  },
  successRatio: {
    labelKey: "usagePerformance.successRatio", format: percent, maximum: 100,
    samples: (metrics) => metrics.succeeded + metrics.failed,
  },
  cacheHitRate: { labelKey: "usagePerformance.cache", format: percent, samples: (metrics) => metrics.cacheSamples, maximum: 100 },
  estimatedCostUsd: { labelKey: "usagePerformance.cost", format: cost, samples: (metrics) => metrics.costSamples },
};

export const RECORD_METRICS: Record<RequestUsageRecordMetric, MetricChoice & { format: ValueFormatter }> = {
  durationMs: { labelKey: "usagePerformance.durationColumn", format: duration },
  firstTokenMs: { labelKey: "usagePerformance.firstColumn", format: duration, generationOnly: true },
  firstContentMs: { labelKey: "usagePerformance.firstContent", format: duration, generationOnly: true },
  outputTokensPerSecond: { labelKey: "usagePerformance.speedColumn", format: speed, generationOnly: true },
  inputTokens: { labelKey: "usagePerformance.inputTokens", format: tokens },
  outputTokens: { labelKey: "usagePerformance.outputTokens", format: tokens },
};

export function chartMetricOptions<K extends string>(
  definitions: Record<K, MetricChoice>, scope: MeasurementScope, t: TFunction,
) {
  return (Object.keys(definitions) as K[])
    .filter((key) => scope === "generation" || !definitions[key].generationOnly)
    .map((value) => ({ value, label: t(definitions[value].labelKey) }));
}
