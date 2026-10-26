import type { ResourceIssue, ResourceMetric as Metric } from "../../api/client";
import { useI18n, type TFunction } from "../../i18n";
import type { MessageKey } from "../../i18n/messages";

const ISSUE_KEYS = {
  warmingUp: "resources.issue.warmingUp", accessDenied: "resources.issue.accessDenied",
  processExited: "resources.issue.processExited", identityUnavailable: "resources.issue.identityUnavailable",
  identityChanged: "resources.issue.identityChanged", counterReset: "resources.issue.counterReset",
  intervalInterrupted: "resources.issue.intervalInterrupted", cpuTopologyChanged: "resources.issue.cpuTopologyChanged",
  collectionFailed: "resources.issue.collectionFailed", unrecognizedExecutable: "resources.issue.unrecognizedExecutable",
  webviewUnavailable: "resources.issue.webviewUnavailable", webviewPartial: "resources.issue.webviewPartial",
  browserExcluded: "resources.issue.browserExcluded", enumerationRestricted: "resources.issue.enumerationRestricted",
  namespaceBoundary: "resources.issue.namespaceBoundary", approximateRss: "resources.issue.approximateRss",
  parentUnverified: "resources.issue.parentUnverified", notObserved: "resources.issue.notObserved",
} as const satisfies Record<ResourceIssue, MessageKey>;

export function resourceIssueText(issue: ResourceIssue, t: TFunction): string {
  return t(ISSUE_KEYS[issue]);
}

type MetricKind = "cpu" | "memory" | "count";

export function formatResourceValue(value: number | null, kind: MetricKind, language: string): string {
  if (value === null) return "—";
  if (kind === "cpu") return `${value.toLocaleString(language, { maximumFractionDigits: 1 })}%`;
  if (kind === "memory") {
    const gib = value >= 1024 ** 3;
    return `${(value / 1024 ** (gib ? 3 : 2)).toLocaleString(language, { maximumFractionDigits: 1 })} ${gib ? "GiB" : "MiB"}`;
  }
  return value.toLocaleString(language, { maximumFractionDigits: 0 });
}

export function ResourceMetric({ metric, kind }: { metric: Metric; kind: MetricKind }) {
  const { t, language } = useI18n();
  const warming = metric.issues.includes("warmingUp") || metric.issues.includes("intervalInterrupted");
  const absent = metric.issues.length === 1 && metric.issues[0] === "notObserved";
  const note = absent ? null : metric.state === "partial" ? t("resources.metric.partial")
    : metric.state === "unavailable" ? warming ? t("resources.metric.warming")
      : resourceIssueText(metric.issues[0] ?? "collectionFailed", t) : null;
  return <div className="asb-resource-metric" title={metric.issues.map((issue) => resourceIssueText(issue, t)).join("\n")}>
    <span>{formatResourceValue(metric.value, kind, language)}</span>
    {note && <span className="asb-resource-note">{note}</span>}
  </div>;
}
