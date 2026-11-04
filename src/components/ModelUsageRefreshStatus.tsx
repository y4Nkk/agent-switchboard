import { useEffect, useId, useState } from "react";
import { useI18n, type MessageKey } from "../i18n";
import type { ModelUsageActivity, ModelUsageRunning } from "../pages/model-usage-reader";
import { modelUsageProgress } from "../pages/model-usage-progress";

const PHASES = {
  discovering: "usage.progress.discovering", checking: "usage.progress.checking",
  reading: "usage.progress.reading", verifying: "usage.progress.verifying",
  aggregating: "usage.progress.aggregating", publishing: "usage.progress.publishing",
} as const satisfies Record<string, MessageKey>;

function bytes(value: number, language: string) {
  const units = ["B", "KB", "MB", "GB"];
  const unit = value > 0 ? Math.min(3, Math.floor(Math.log10(value) / 3)) : 0;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: unit ? 1 : 0 }).format(value / 1000 ** unit)} ${units[unit]}`;
}

export function ModelUsageRefreshStatus({ activity, task, hasReport }: {
  activity: ModelUsageActivity; task: ModelUsageRunning | null; hasReport: boolean;
}) {
  const { t, language } = useI18n();
  const id = useId();
  const [seconds, setSeconds] = useState(0);
  const startedAt = task?.startedAt;
  useEffect(() => {
    const started = startedAt ? new Date(startedAt).getTime() : Date.now();
    const tick = () => setSeconds(Math.max(0, Math.floor((Date.now() - started) / 1_000)));
    tick();
    if (activity === null) return undefined;
    const timer = setInterval(tick, 1_000);
    return () => clearInterval(timer);
  }, [activity, startedAt]);
  if (activity === null) return null;
  const metric = modelUsageProgress(task?.progress);
  const label = activity === "snapshot" ? t("usage.status.readingSnapshot")
    : task ? t(PHASES[task.progress.phase]) : t("usage.progress.starting");
  const detail = metric ? t(`usage.progress.${metric.unit}`, {
    done: metric.unit === "bytes" ? bytes(metric.done, language) : metric.done.toLocaleString(language),
    total: metric.unit === "bytes" ? bytes(metric.total, language) : metric.total.toLocaleString(language),
  }) : t("usage.progress.preparing");
  return <div className="asb-usage-progress" data-initial={!hasReport}>
    <div className="asb-usage-progress-heading">
      {metric ? <svg className="asb-usage-ring" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="12" r="9" className="asb-usage-ring-track" />
        <circle cx="12" cy="12" r="9" pathLength="100" strokeDasharray="100"
          strokeDashoffset={100 - metric.percent} className="asb-usage-ring-value" />
      </svg> : <span className="asb-usage-spinner" aria-hidden="true" />}
      <span id={id} role="status">{label}</span>
      {metric && <span className="asb-num asb-usage-progress-percent" aria-hidden="true">{metric.percent}%</span>}
    </div>
    <div className="asb-usage-progress-track" role="progressbar" aria-labelledby={id}
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={metric?.percent}
      aria-valuetext={`${label} · ${detail}`} data-indeterminate={metric === null}>
      <span style={metric ? { width: `${metric.percent}%` } : undefined} />
    </div>
    <div className="asb-usage-progress-detail asb-num">
      <span>{detail}</span><span role="timer" aria-live="off">{t("usage.status.elapsed", { seconds })}</span>
    </div>
    {task?.progress.totalSources !== null && task?.progress.totalSources !== undefined && metric?.unit !== "sources" &&
      <p className="asb-usage-progress-note">{t("usage.progress.sources", {
        done: task.progress.completedSources.toLocaleString(language), total: task.progress.totalSources.toLocaleString(language),
      })}</p>}
    <p className="asb-usage-progress-note">{t(hasReport ? "usage.status.updatingSaved" : "usage.loading.firstRun")}</p>
  </div>;
}
