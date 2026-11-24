import { useEffect, useId, useState } from "react";
import type {
  SkillRepositoryRequest, SkillRepositoryRequestStep, SkillRepositoryScanProgress, SkillRepositoryScanRow,
} from "../../api/extensions/skill-sources";
import { useI18n, type MessageKey } from "../../i18n";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { skillRepositoryLabel } from "./skill-repository-model";
import { sourceErrorMessage } from "./skill-source-model";

const STEPS = ["commit", "tree", "archive"] as const;
const STEP_LABELS = {
  commit: "extensions.repoScan.commit", tree: "extensions.repoScan.tree", archive: "extensions.repoScan.archive",
} as const satisfies Record<SkillRepositoryRequestStep, MessageKey>;
const REQUEST_LABELS = {
  waiting: "extensions.repoScan.waiting", reading: "extensions.repoScan.reading",
  received: "extensions.repoScan.received", failed: "extensions.repoScan.requestFailed",
} as const satisfies Record<SkillRepositoryRequest["state"], MessageKey>;

function formatBytes(bytes: number, language: string) {
  const units = ["B", "KB", "MB"];
  const unit = bytes >= 1_000_000 ? 2 : bytes >= 1_000 ? 1 : 0;
  const value = (bytes / 1000 ** unit).toLocaleString(language, { maximumFractionDigits: 1 });
  return `${value} ${units[unit]}`;
}

function RequestStep({ step, request, pending }: {
  step: SkillRepositoryRequestStep; request: SkillRepositoryRequest | undefined; pending: boolean;
}) {
  const { t, language } = useI18n();
  const total = request?.totalBytes ?? 0;
  const bytes = request?.receivedBytes ?? 0;
  const status = request?.httpStatus;
  const tone = request?.state === "failed" || (status != null && status !== 200) ? "warning"
    : request?.state === "received" ? "complete" : request ? "active" : "idle";
  const fill = request?.state === "received" ? 100 : total > 0 ? Math.min(100, bytes / total * 100) : 0;
  const detail = request?.state === "failed" && status == null ? t("extensions.repoScan.noResponse")
    : request ? t(REQUEST_LABELS[request.state]) : t(pending ? "extensions.repoScan.queued" : "extensions.repoScan.skipped");

  return <li className="asb-repo-scan-step" data-tone={tone}>
    <div className="asb-repo-scan-step-track" aria-hidden="true"><span style={{ width: `${fill}%` }} /></div>
    <div className="asb-repo-scan-step-title">
      <span>{t(STEP_LABELS[step])}</span>
      {status != null && <span className="asb-repo-scan-http asb-num">HTTP {status}</span>}
    </div>
    <p className="asb-repo-scan-step-detail asb-num">
      {detail}{status != null && <> · {formatBytes(bytes, language)}{total > 0 && ` / ${formatBytes(total, language)}`}</>}
    </p>
  </li>;
}

function RepositoryScanRow({ row, previous }: { row: SkillRepositoryScanRow; previous: boolean }) {
  const { t } = useI18n();
  const pending = row.state === "queued" || row.state === "running";
  const latest = row.requests.at(-1);
  const status = row.state === "complete" ? t("extensions.repoScan.candidateCount", { count: row.candidateCount })
    : row.state === "failed" ? t("extensions.sources.refreshFailed")
    : row.state === "queued" ? t("extensions.repoScan.repoQueued")
    : latest?.state === "received" && latest.httpStatus === 200 ? t("extensions.repoScan.verifying")
    : t("extensions.sources.scanning");

  return <li className="asb-repo-scan-row" data-state={row.state}>
    <div className="asb-repo-scan-row-heading">
      <span className="asb-repo-scan-name">{skillRepositoryLabel(row.repository)}</span>
      <span className="asb-repo-scan-row-status asb-num">{status}</span>
    </div>
    <ol className="asb-repo-scan-steps">
      {STEPS.map((step) => <RequestStep key={step} step={step} pending={pending}
        request={row.requests.find((request) => request.step === step)} />)}
    </ol>
    {row.state === "failed" && <p className="asb-repo-scan-error">
      {sourceErrorMessage(row.message, t("extensions.sources.refreshFailed"))}
      {previous && <> · {t("extensions.repoScan.previousResults")}</>}
    </p>}
  </li>;
}

export function SkillRepositoryScanPanel({ progress, running, unavailable, hasPrevious }: {
  progress: SkillRepositoryScanProgress | null;
  running: boolean;
  unavailable: boolean;
  hasPrevious: (id: string) => boolean;
}) {
  const { t, language } = useI18n();
  const contentId = useId();
  const [open, setOpen] = useState(true);
  useEffect(() => { if (running) setOpen(true); }, [running]);
  const rows = progress?.repositories ?? [];
  const done = rows.filter((row) => row.state === "complete" || row.state === "failed").length;
  const reached = rows.filter((row) => row.requests.some((request) => request.httpStatus !== null)).length;
  const title = t("extensions.repoScan.title");
  const summary = progress ? t("extensions.repoScan.processed", { done, total: rows.length })
    : t("extensions.repoScan.preparing");

  return <section className="asb-repo-scan" aria-label={title}>
    <div className="asb-repo-scan-heading">
      {rows.length > 0 ? <CollapsibleTrigger className="asb-repo-scan-trigger" open={open} onOpenChange={setOpen}
        contentId={contentId} label={title} /> : <h3 className="asb-section-title">{title}</h3>}
      <span className="asb-num" role="status">{summary}</span>
      {rows.length > 0 && <span className="asb-num">{t("extensions.repoScan.responses", { count: reached })}</span>}
      {progress && <span className="asb-num" aria-live="off">{t("extensions.repoScan.elapsed", {
        seconds: (progress.elapsedMs / 1_000).toLocaleString(language, { maximumFractionDigits: 1 }),
      })}</span>}
    </div>
    {rows.length > 0 && <progress className="asb-repo-scan-meter" value={done} max={rows.length}
      aria-label={title} aria-valuetext={summary} />}
    {unavailable && <p className="asb-repo-scan-error" role="status">{t("extensions.repoScan.readUnavailable")}</p>}
    {rows.length > 0 && <Collapsible open={open} id={contentId}>
      <ol className="asb-repo-scan-rows">
        {rows.map((row) => <RepositoryScanRow key={row.repository.id} row={row}
          previous={hasPrevious(row.repository.id)} />)}
      </ol>
    </Collapsible>}
  </section>;
}
