import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { HubConfidence, HubEvent, HubIncident, HubSurface, HubSurfaceStatus, ResetType } from "../api/client";
import type { MessageKey, TFunction } from "../i18n";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { CollapsibleSection } from "./Collapsible";
import { Time } from "./Time";
import { relativeLabel } from "../lib/time";
import { UpdateIcon } from "./icons";
import { ResetBadge } from "./CodexResetPanel";
import type { useCodexResetSignal } from "./quota-reads";
import { Table } from "./Table";

type HubRead = ReturnType<typeof useCodexResetSignal>["hub"];

const CONFIDENCE_LABELS: Record<HubConfidence, MessageKey> = {
  low: "codex.reset.hub.confidenceLow",
  medium: "codex.reset.hub.confidenceMedium",
  high: "codex.reset.hub.confidenceHigh",
};

const SURFACE_LABELS: Record<HubSurfaceStatus, MessageKey> = {
  operational: "codex.reset.hub.surfaceOperational",
  degraded: "codex.reset.hub.surfaceDegraded",
  outage: "codex.reset.hub.surfaceOutage",
  maintenance: "codex.reset.hub.surfaceMaintenance",
};

function impactLabel(impact: string, t: TFunction): string {
  switch (impact) {
    case "none":
      return t("codex.reset.hub.impactNone");
    case "minor":
      return t("codex.reset.hub.impactMinor");
    case "major":
      return t("codex.reset.hub.impactMajor");
    case "critical":
      return t("codex.reset.hub.impactCritical");
    default:
      return impact;
  }
}

/** Day-scale decimals come straight from the source (1.8 days); whole days
 * stay integers instead of reading "2.0 天". */
function formatDays(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function rangePercent(lower: number, upper: number): string {
  return `${Math.round(lower * 100)}–${Math.round(upper * 100)}%`;
}

function eventBadgeType(kind: HubEvent["kind"]): ResetType {
  return kind === "reset" ? "global" : "banked";
}

function openLink(url: string): void {
  void openUrl(url);
}

function SurfacePill({ surface }: { surface: HubSurface }) {
  const { t } = useI18n();
  return (
    <span className={`asb-status-pill${surface.status === "operational" ? " is-ok" : ""}`}>
      <span className="asb-status-pill-dot" aria-hidden="true" />
      <span>{surface.label} {t(SURFACE_LABELS[surface.status])}</span>
    </span>
  );
}

function PostLink({ url }: { url: string | null }) {
  const { t } = useI18n();
  if (url === null) return <>—</>;
  return (
    <a className="asb-codex-reset-source-link" href={url}
      onClick={(event) => {
        event.preventDefault();
        openLink(url);
      }}>
      {t("codex.reset.viewPost")}
    </a>
  );
}

/** Source 2: codex-reset.com's aggregate answer — probability forecast,
 * reset timeline, OpenAI platform status and the radar highlights. The
 * refresh action and freshness state live in the usage page header. */
export function CodexResetHubPanel({ read }: { read: HubRead }) {
  const { t } = useI18n();
  const [incidentsOpen, setIncidentsOpen] = useState(false);
  const { snapshot, cacheLoading, cacheError, readError } = read;

  const status = snapshot?.status ?? null;
  const cacheWarning = snapshot?.cacheWarning ?? null;
  const isCachedRead = snapshot?.freshness === "cached";
  const forecast = status?.forecast ?? null;

  return (
    <>
      {cacheLoading && status === null && <div className="asb-quota-loading" role="status" aria-label={t("codex.reset.loadingCache")}>
        <span className="asb-skeleton" aria-hidden="true" /><span className="asb-skeleton" aria-hidden="true" /></div>}
      {status === null && !cacheLoading && readError === null && (
        <div className="asb-empty-state">
          <span className="asb-empty-state-icon" aria-hidden="true">
            <UpdateIcon />
          </span>
          <h3 className="asb-section-title">{t("codex.reset.empty")}</h3>
        </div>
      )}
      {cacheError && <p className="asb-warn-text" role="alert">{t("codex.reset.cacheUnavailable", { error: cacheError })}</p>}
      {readError && (
        <p className="asb-warn-text" role="alert">
          {t("codex.reset.readFailed", { error: readError })}
          {status !== null ? t("codex.reset.showingStale") : ""}
        </p>
      )}
      {status !== null && forecast !== null && (
        <>
          <div className="asb-codex-reset-board">
            <article className="asb-codex-reset-summary" aria-label={t("codex.reset.hub.forecastAria")}>
              <p className="asb-codex-reset-question">{t("codex.reset.question")}</p>
              <strong className="asb-codex-reset-answer is-forecast">
                {t("codex.reset.hub.answer24h", { percent: forecast.chance24h.percent })}
              </strong>
              <p className="asb-codex-reset-detail">
                {t("codex.reset.hub.answer48h", {
                  percent: forecast.chance48h.percent,
                  range: rangePercent(forecast.chance48h.rangeLower, forecast.chance48h.rangeUpper),
                })}
              </p>
              <p className="asb-codex-reset-detail">
                {t(CONFIDENCE_LABELS[forecast.confidence])}
                {forecast.window !== null && (
                  <>
                    {" · "}
                    {t("codex.reset.hub.windowValue", {
                      start: forecast.window.startHour,
                      end: forecast.window.endHour,
                      timezone: forecast.window.timezone,
                    })}
                  </>
                )}
              </p>
              {forecast.contextNote !== null && <p className="asb-codex-reset-detail">{forecast.contextNote}</p>}
              {status.feed.signal && (
                <div className="asb-codex-reset-post-summary">
                  <p className="asb-codex-reset-label">{t("codex.reset.hub.signalLabel")}</p>
                  <p className="asb-codex-reset-post">{status.feed.signal.summary}</p>
                  <div className="asb-codex-reset-post-actions">
                    <span className="asb-codex-reset-detail">
                      <Time iso={status.feed.signal.at} />
                    </span>
                    <Button variant="secondary" onClick={() => openLink(status.feed.signal!.url)}>
                      {t("codex.reset.viewPost")}
                    </Button>
                  </div>
                </div>
              )}
            </article>
            <dl className="asb-codex-reset-facts" aria-label={t("codex.reset.hub.factsAria")}>
              <div className="asb-codex-reset-fact">
                <dt>{t("codex.reset.lastCompleted")}</dt>
                <dd>
                  <span>
                    <Time iso={forecast.lastResetAt} />
                    <span className="asb-codex-reset-relative">
                      （{relativeLabel(forecast.lastResetAt)}）
                    </span>
                  </span>
                </dd>
              </div>
              {forecast.wait !== null && (
                <div className="asb-codex-reset-fact">
                  <dt>{t("codex.reset.hub.wait")}</dt>
                  <dd>
                    <span className="asb-codex-reset-detail">
                      {t("codex.reset.hub.waitValue", {
                        days: formatDays(forecast.wait.days),
                        sample: forecast.wait.sample,
                        share: Math.round(forecast.wait.shorterShare * 100),
                        median: formatDays(forecast.wait.medianDays),
                      })}
                    </span>
                  </dd>
                </div>
              )}
              {forecast.cadence !== null && (
                <div className="asb-codex-reset-fact">
                  <dt>{t("codex.reset.hub.cadence")}</dt>
                  <dd>
                    <span className="asb-codex-reset-detail">
                      {t("codex.reset.hub.cadenceValue", {
                        sample: forecast.cadence.sample,
                        median: formatDays(forecast.cadence.medianDays),
                        mean: formatDays(forecast.cadence.meanDays),
                      })}
                      {forecast.cadence.accelerating ? ` · ${t("codex.reset.hub.cadenceAccelerating")}` : ""}
                    </span>
                  </dd>
                </div>
              )}
            </dl>
          </div>
          <div className="asb-codex-reset-section">
            <div className="asb-codex-reset-section-head">
              <p className="asb-codex-reset-section-title">{t("codex.reset.hub.timelineTitle")}</p>
              <p className="asb-codex-reset-section-total">
                {t("codex.reset.hub.timelineTotal", { resets: status.timeline.resets, credits: status.timeline.credits })}
              </p>
            </div>
            <Table rows={status.timeline.events} rowKey={(event) => event.id}
              ariaLabel={t("codex.reset.hub.timelineAria")} className="asb-codex-reset-timeline-table" columns={[
                { key: "date", header: t("codex.reset.dailyDate"), render: (event) => event.date },
                { key: "kind", header: t("codex.reset.hub.colKind"), render: (event) => <ResetBadge type={eventBadgeType(event.kind)} /> },
                { key: "summary", header: t("codex.reset.hub.colSummary"), render: (event) => (
                  <>
                    {event.summary === "" ? "—" : event.summary}
                    {!event.confirmed && (
                      <span className="asb-codex-reset-relative">（{t("codex.reset.hub.unconfirmed")}）</span>
                    )}
                  </>
                ) },
                { key: "post", header: t("codex.reset.hub.colPost"), render: (event) => <PostLink url={event.url} /> },
              ]} />
          </div>
          <div className="asb-codex-reset-section">
            <div className="asb-codex-reset-section-head">
              <p className="asb-codex-reset-section-title">{t("codex.reset.hub.platformTitle")}</p>
              <p className="asb-codex-reset-section-total"><Time iso={status.platform.checkedAt} /></p>
            </div>
            <div className="asb-codex-reset-surfaces">
              {status.platform.surfaces.map((surface) => (
                <SurfacePill key={surface.id} surface={surface} />
              ))}
            </div>
            <p className="asb-codex-reset-detail">
              {t("codex.reset.hub.platformStats", {
                days: status.platform.stats.days,
                incidents: status.platform.stats.incidents,
                open: status.platform.stats.open,
              })}
              {status.platform.stats.medianRecoveryMinutes !== null && (
                <>
                  {" · "}
                  {t("codex.reset.hub.medianRecovery", {
                    median: Math.round(status.platform.stats.medianRecoveryMinutes),
                  })}
                </>
              )}
            </p>
            <CollapsibleSection quiet open={incidentsOpen} onOpenChange={setIncidentsOpen}
              label={t("codex.reset.hub.incidentsLabel")}>
              <Table rows={status.platform.incidents}
                rowKey={(incident: HubIncident, index: number) => `${incident.startedAt}-${index}`}
                ariaLabel={t("codex.reset.hub.incidentsAria")} className="asb-codex-reset-incidents-table" columns={[
                  { key: "name", header: t("codex.reset.hub.colName"), render: (incident) => incident.name },
                  { key: "impact", header: t("codex.reset.hub.colImpact"), render: (incident) => impactLabel(incident.impact, t) },
                  { key: "started", header: t("codex.reset.hub.colStarted"), render: (incident) => <Time iso={incident.startedAt} /> },
                  { key: "resolved", header: t("codex.reset.hub.colResolved"), render: (incident) =>
                    incident.resolvedAt !== null ? <Time iso={incident.resolvedAt} /> : t("codex.reset.hub.ongoing") },
                ]} />
            </CollapsibleSection>
          </div>
          {status.feed.banked.length > 0 && (
            <div className="asb-codex-reset-section">
              <div className="asb-codex-reset-section-head">
                <p className="asb-codex-reset-section-title">{t("codex.reset.hub.bankedTitle")}</p>
              </div>
              <Table rows={status.feed.banked} rowKey={(note) => note.at}
                ariaLabel={t("codex.reset.hub.bankedAria")} className="asb-codex-reset-banked-table" columns={[
                  { key: "at", header: t("codex.reset.hub.colAt"), render: (note) => <Time iso={note.at} /> },
                  { key: "text", header: t("codex.reset.hub.colNote"), render: (note) => note.text },
                ]} />
            </div>
          )}
          {status.sourceWarning && <p className="asb-warn-text">{status.sourceWarning}</p>}
          {cacheWarning && <p className="asb-warn-text">{cacheWarning}</p>}
          <p className="asb-codex-reset-source">
            {t("codex.reset.hub.generated")} <Time iso={status.generatedAt} /> · {isCachedRead ? t("codex.reset.cachedAt") : t("codex.reset.readThisTime")} <Time iso={status.checkedAt} />
          </p>
          <p className="asb-codex-reset-note">
            {t("codex.reset.hub.sourceNoteLead")}{" "}
            <a
              className="asb-codex-reset-source-link"
              href={status.sourceUrl}
              onClick={(event) => {
                event.preventDefault();
                openLink(status.sourceUrl);
              }}
            >
              {status.sourceUrl}
            </a>
            {t("codex.reset.sourceNoteTail")}
          </p>
        </>
      )}
    </>
  );
}
