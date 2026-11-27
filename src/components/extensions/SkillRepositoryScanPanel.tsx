import { FileArchive, FolderTree, GitCommitHorizontal, type LucideIcon } from "lucide-react";
import { useEffect, useId, useState, type ReactNode } from "react";
import type {
  SkillRepositoryRequest, SkillRepositoryRequestStep, SkillRepositoryScanProgress, SkillRepositoryScanRow,
} from "../../api/extensions/skill-sources";
import { useI18n, type MessageKey, type TFunction } from "../../i18n";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { FactList, FactStatus, type FactListRow } from "../FactList";
import { SkillRepositoryFailure } from "./SkillRepositoryRecovery";
import { skillRepositoryLabel } from "./skill-repository-model";

const REQUEST_STEPS = {
  commit: { label: "extensions.repoScan.commit", Icon: GitCommitHorizontal },
  tree: { label: "extensions.repoScan.tree", Icon: FolderTree },
  archive: { label: "extensions.repoScan.archive", Icon: FileArchive },
} as const satisfies Record<SkillRepositoryRequestStep, { label: MessageKey; Icon: LucideIcon }>;
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

function requestReadout(request: SkillRepositoryRequest, t: TFunction, language: string) {
  const { state, step, httpStatus, receivedBytes, totalBytes } = request;
  const notModified = state === "received" && step === "commit" && httpStatus === 304;
  const label = t(notModified ? "extensions.repoScan.notModified"
    : state === "failed" && httpStatus === null ? "extensions.repoScan.noResponse" : REQUEST_LABELS[state]);
  if (httpStatus === null || notModified) return { label, bytes: null };

  const received = formatBytes(receivedBytes, language);
  const bytes = totalBytes === null ? received : `${received} / ${formatBytes(totalBytes, language)}`;
  return { label, bytes };
}

function repositoryStatus(row: SkillRepositoryScanRow, t: TFunction, language: string) {
  if (row.state === "queued") return t("extensions.repoScan.repoQueued");
  if (row.state === "complete") return t("extensions.repoScan.candidateCount", { count: row.candidateCount });
  if (row.state === "failed") return t("extensions.sources.refreshFailed");

  const latest = row.requests.at(-1);
  if (!latest) return t("extensions.sources.scanning");
  const stage = t(REQUEST_STEPS[latest.step].label);
  if (latest.state === "received" && latest.httpStatus === 200) {
    return `${stage} · ${t("extensions.repoScan.verifying")}`;
  }
  const { label, bytes } = requestReadout(latest, t, language);
  const http = latest.httpStatus === null ? null : `HTTP ${latest.httpStatus}`;
  return [stage, label, bytes, http].filter(Boolean).join(" · ");
}

function RepositoryRequests({ row }: { row: SkillRepositoryScanRow }) {
  const { t, language } = useI18n();
  const missing = t(row.state === "running" ? "extensions.repoScan.queued" : "extensions.repoScan.skipped");
  const facts = Object.entries(REQUEST_STEPS).map<FactListRow>(([step, { label, Icon }]) => {
    const request = row.requests.find((entry) => entry.step === step);
    const readout = request && requestReadout(request, t, language);
    const http = request?.httpStatus;
    return {
      label: t(label),
      icon: Icon,
      tone: request?.state === "failed" ? "warning"
        : request?.state === "waiting" || request?.state === "reading" ? "active" : undefined,
      value: <span className="asb-repo-scan-readout" data-state={request?.state}>
        <span className="asb-repo-scan-step-state">{readout?.label ?? missing}</span>{" "}
        {readout?.bytes != null && <span className="asb-repo-scan-bytes asb-num">{readout.bytes}</span>}{" "}
        {http != null && <FactStatus tone={http === 304 ? "neutral" : http >= 200 && http < 300 ? "success" : "warning"}>
          HTTP&nbsp;{http}
        </FactStatus>}
      </span>,
    };
  });
  return <div className="asb-repo-scan-requests"><FactList rows={facts} /></div>;
}

function RepositoryScanRow({ row, previous }: { row: SkillRepositoryScanRow; previous: boolean }) {
  const { t, language } = useI18n();
  const contentId = useId();
  const [open, setOpen] = useState(false);
  const hasDetails = row.requests.length > 0 || row.state === "complete" || row.state === "failed";
  const heading = <span className="asb-repo-scan-row-heading">
    <span className="asb-repo-scan-row-copy">
      <span className="asb-repo-scan-name">{skillRepositoryLabel(row.repository)}</span>
      <span className="asb-repo-scan-row-status asb-num">{repositoryStatus(row, t, language)}</span>
    </span>
  </span>;

  return <li className="asb-repo-scan-row" data-state={row.state}>
    {hasDetails ? <CollapsibleTrigger className="asb-repo-scan-row-trigger" open={open} onOpenChange={setOpen}
      contentId={contentId} label={heading} /> : <div className="asb-repo-scan-row-static">{heading}</div>}
    {row.state === "failed" && <SkillRepositoryFailure row={row} previous={previous} className="asb-repo-scan-error" />}
    {hasDetails && <Collapsible open={open} id={contentId}><RepositoryRequests row={row} /></Collapsible>}
  </li>;
}

export function SkillRepositoryScanPanel({ progress, running, unavailable, hasPrevious, recoveryAction }: {
  progress: SkillRepositoryScanProgress | null;
  running: boolean;
  unavailable: boolean;
  hasPrevious: (id: string) => boolean;
  recoveryAction: ReactNode;
}) {
  const { t, language } = useI18n();
  const contentId = useId();
  const [open, setOpen] = useState(true);
  useEffect(() => { if (running) setOpen(true); }, [running]);
  const rows = progress?.repositories ?? [];
  const total = rows.length;
  const failed = rows.filter((row) => row.state === "failed").length;
  const done = failed + rows.filter((row) => row.state === "complete").length;
  const portion = total > 0 ? done / total * 100 : 22;
  const spinning = running && progress === null && !unavailable;
  const title = t("extensions.repoScan.title");
  const summary = progress ? t("extensions.repoScan.processed", { done, total })
    : t("extensions.repoScan.preparing");

  return <section className="asb-repo-scan" aria-label={title}>
    <div className="asb-repo-scan-heading">
      {(total > 0 || progress === null) && <div
        className={`asb-repo-scan-ring${spinning ? " asb-skill-source-spinner" : ""}`}
        data-tone={unavailable ? "muted" : !running && failed > 0 ? "warning" : undefined}
        style={{ backgroundImage: `conic-gradient(currentColor ${portion}%, var(--asb-hairline-strong) ${portion}%)` }}
        role="progressbar" aria-label={title} aria-valuemin={0} aria-valuemax={total || undefined}
        aria-valuenow={total ? done : undefined} aria-valuetext={summary} />}
      <div className="asb-repo-scan-summary">
        <h3 className="asb-section-title">
          {total > 0 ? <CollapsibleTrigger className="asb-repo-scan-trigger" open={open} onOpenChange={setOpen}
            contentId={contentId} label={title} /> : title}
        </h3>
        <p className="asb-repo-scan-stats asb-num">
          <span role="status">{summary}{failed > 0 && <span className="asb-repo-scan-failures">
            {" · "}{t("extensions.repoScan.failedCount", { count: failed })}
          </span>}</span>
          {progress && <span>{t("extensions.repoScan.elapsed", {
            seconds: (progress.elapsedMs / 1_000).toLocaleString(language, { maximumFractionDigits: 1 }),
          })}</span>}
        </p>
      </div>
      {open && recoveryAction}
    </div>
    {unavailable && <p className="asb-repo-scan-error" role="status">{t("extensions.repoScan.readUnavailable")}</p>}
    {total > 0 && <Collapsible open={open} id={contentId}>
      <ol className="asb-repo-scan-rows" role="list">
        {rows.map((row) => <RepositoryScanRow key={row.repository.id} row={row}
          previous={hasPrevious(row.repository.id)} />)}
      </ol>
    </Collapsible>}
  </section>;
}
