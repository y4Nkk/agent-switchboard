import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Clock3 } from "lucide-react";
import type {
  CodexResetHubStatus, HubConfidence, HubEvent, HubFeed, HubForecast,
  HubPlatform, HubSurface, HubSurfaceStatus, HubTimeline, ResetType,
} from "../api/client";
import type { MessageKey, TFunction } from "../i18n";
import { useI18n } from "../i18n";
import { CollapsibleSection } from "./Collapsible";
import { FactList } from "./FactList";
import { Time } from "./Time";
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

function HubSourceMeta({ status, cached }: { status: CodexResetHubStatus; cached: boolean }) {
  const { t } = useI18n();
  const { sourceUrl, generatedAt, checkedAt } = status;
  return (
    <div className="asb-codex-reset-source asb-codex-reset-detail">
      <a className="asb-codex-reset-source-link" href={sourceUrl} onClick={(event) => {
        event.preventDefault();
        openLink(sourceUrl);
      }}>{sourceUrl.replace(/^https:\/\//, "")}</a>
      <span>{t("codex.reset.hub.sourceUpdated")} <Time iso={generatedAt} /></span>
      <span>{t(cached ? "codex.reset.hub.cachedSnapshot" : "codex.reset.hub.fetchedAt")} <Time iso={checkedAt} /></span>
    </div>
  );
}

function HubActivity({ latestEvent, lastResetAt, signal }: {
  latestEvent: HubEvent | undefined; lastResetAt: string; signal: HubFeed["signal"];
}) {
  const { t } = useI18n();
  return (
    <section className="asb-codex-reset-summary asb-codex-reset-hub-activity">
      <h4 className="asb-codex-reset-summary-title">{t("codex.reset.hub.latestActivity")}</h4>
      {latestEvent ? (
        <div className="asb-codex-reset-hub-event">
          <div className="asb-codex-reset-hub-event-meta asb-codex-reset-detail">
            <ResetBadge type={eventBadgeType(latestEvent.kind)} />
            <Time iso={latestEvent.announcedAt} />
            {!latestEvent.confirmed && <span>{t("codex.reset.hub.unconfirmed")}</span>}
            {latestEvent.url && <PostLink url={latestEvent.url} />}
          </div>
          {latestEvent.summary.trim() !== "" && <p className="asb-codex-reset-post">{latestEvent.summary}</p>}
        </div>
      ) : (
        <p className="asb-codex-reset-detail">{t("codex.reset.hub.noActivity")}</p>
      )}
      <FactList inline rows={[{ label: t("codex.reset.hub.lastGlobalReset"), icon: Clock3,
        value: <Time iso={lastResetAt} /> }]} />
      {signal && (
        <div className="asb-codex-reset-hub-event">
          <div className="asb-codex-reset-hub-event-meta asb-codex-reset-detail">
            <span className="asb-codex-reset-label">{t("codex.reset.hub.signalLabel")}</span>
            <Time iso={signal.at} />
            <PostLink url={signal.url} />
          </div>
          <p className="asb-codex-reset-post">{signal.summary}</p>
        </div>
      )}
    </section>
  );
}

function HubForecastDetails({ forecast }: { forecast: HubForecast }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const { wait, cadence, contextNote } = forecast;
  if (!wait && !cadence && !contextNote) return null;

  return (
    <CollapsibleSection quiet open={open} onOpenChange={setOpen} label={t("codex.reset.hub.forecastDetails")}>
      <div className="asb-codex-reset-hub-evidence">
        {(wait || cadence) && (
          <dl className="asb-codex-reset-facts">
            {wait && (
              <div className="asb-codex-reset-fact">
                <dt>{t("codex.reset.hub.wait")}</dt>
                <dd className="asb-codex-reset-detail">{t("codex.reset.hub.waitValue", {
                  days: formatDays(wait.days), sample: wait.sample,
                  share: Math.round(wait.shorterShare * 100), median: formatDays(wait.medianDays),
                })}</dd>
              </div>
            )}
            {cadence && (
              <div className="asb-codex-reset-fact">
                <dt>{t("codex.reset.hub.cadence")}</dt>
                <dd className="asb-codex-reset-detail">
                  {t("codex.reset.hub.cadenceValue", {
                    sample: cadence.sample, median: formatDays(cadence.medianDays), mean: formatDays(cadence.meanDays),
                  })}
                  {cadence.accelerating && <> · {t("codex.reset.hub.cadenceAccelerating")}</>}
                </dd>
              </div>
            )}
          </dl>
        )}
        {contextNote && <p className="asb-codex-reset-detail" lang="zh-CN">{contextNote}</p>}
      </div>
    </CollapsibleSection>
  );
}

function HubForecastSummary({ forecast }: { forecast: HubForecast }) {
  const { t } = useI18n();
  const chances = [
    ["codex.reset.hub.horizon24h", forecast.chance24h],
    ["codex.reset.hub.horizon48h", forecast.chance48h],
  ] as const;
  const { window: resetWindow } = forecast;
  return (
    <section className="asb-codex-reset-summary">
      <div className="asb-codex-reset-section-head">
        <h4 className="asb-codex-reset-summary-title">{t("codex.reset.hub.forecastTitle")}</h4>
        <span className="asb-codex-reset-detail">{t(CONFIDENCE_LABELS[forecast.confidence])}</span>
      </div>
      <dl className="asb-codex-reset-hub-horizons">
        {chances.map(([label, chance]) => (
          <div key={label}>
            <dt className="asb-codex-reset-label">{t(label)}</dt>
            <dd className="asb-codex-reset-answer">{chance.percent}%</dd>
            <dd className="asb-codex-reset-detail">{t("codex.reset.hub.probabilityRange", {
              range: rangePercent(chance.rangeLower, chance.rangeUpper),
            })}</dd>
          </div>
        ))}
      </dl>
      {resetWindow && (
        <p className="asb-codex-reset-detail">{t("codex.reset.hub.windowValue", {
          start: resetWindow.startHour,
          end: resetWindow.endHour,
          timezone: resetWindow.timezone,
        })}</p>
      )}
      <HubForecastDetails forecast={forecast} />
    </section>
  );
}

function HubTimelineHistory({ timeline }: { timeline: HubTimeline }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const { events, resets, credits } = timeline;
  if (events.length === 0) return null;

  const title = t("codex.reset.hub.timelineTitle");
  return (
    <CollapsibleSection open={open} onOpenChange={setOpen} label={title}
      value={t("codex.reset.hub.timelineTotal", { resets, credits })}>
      {events.length < resets + credits && (
        <p className="asb-codex-reset-detail">{t("codex.reset.hub.timelineLatest", { count: events.length })}</p>
      )}
      <Table rows={events} rowKey={(event) => event.id}
        ariaLabel={title} className="asb-codex-reset-timeline-table" columns={[
          { key: "at", header: t("codex.reset.hub.colAt"), render: (event) => <Time iso={event.announcedAt} /> },
          { key: "kind", header: t("codex.reset.hub.colKind"), render: (event) => <ResetBadge type={eventBadgeType(event.kind)} /> },
          { key: "summary", header: t("codex.reset.hub.colSummary"), render: (event) => (
            <>
              {event.summary.trim() || "—"}
              {!event.confirmed && (
                <span className="asb-codex-reset-relative">（{t("codex.reset.hub.unconfirmed")}）</span>
              )}
            </>
          ) },
          { key: "post", header: t("codex.reset.hub.colPost"), render: (event) => <PostLink url={event.url} /> },
        ]} />
    </CollapsibleSection>
  );
}

function HubBankedHistory({ observations }: { observations: HubFeed["banked"] }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [latest] = observations;
  if (!latest) return null;

  const title = t("codex.reset.hub.bankedTitle");
  return (
    <CollapsibleSection open={open} onOpenChange={setOpen} label={title}
      value={<>{t("codex.reset.hub.latestObserved")} <Time iso={latest.at} /></>}>
      <Table rows={observations} rowKey={(note) => note.at}
        ariaLabel={title} className="asb-codex-reset-banked-table" columns={[
          { key: "at", header: t("codex.reset.hub.colAt"), render: (note) => <Time iso={note.at} /> },
          { key: "text", header: t("codex.reset.hub.colNote"), render: (note) => note.text },
        ]} />
    </CollapsibleSection>
  );
}

function HubPlatformStatus({ platform }: { platform: HubPlatform }) {
  const { t } = useI18n();
  const [incidentsOpen, setIncidentsOpen] = useState(false);
  const { stats } = platform;
  return (
    <section>
      <div className="asb-codex-reset-section-head">
        <h4 className="asb-codex-reset-summary-title">{t("codex.reset.hub.platformTitle")}</h4>
        <p className="asb-codex-reset-detail">{t("codex.reset.lastCheck")} <Time iso={platform.checkedAt} /></p>
      </div>
      <div className="asb-codex-reset-surfaces">
        {platform.surfaces.map((surface) => (
          <SurfacePill key={surface.id} surface={surface} />
        ))}
      </div>
      <p className="asb-codex-reset-detail">
        {t("codex.reset.hub.platformStats", { days: stats.days, incidents: stats.incidents, open: stats.open })}
        {stats.medianRecoveryMinutes !== null && (
          <> · {t("codex.reset.hub.medianRecovery", { median: Math.round(stats.medianRecoveryMinutes) })}</>
        )}
      </p>
      {platform.incidents.length > 0 && (
        <CollapsibleSection quiet open={incidentsOpen} onOpenChange={setIncidentsOpen}
          label={t("codex.reset.hub.incidentsLabel")}>
          <Table rows={platform.incidents} rowKey={(incident, index) => incident.startedAt + "-" + index}
            ariaLabel={t("codex.reset.hub.incidentsAria")} className="asb-codex-reset-incidents-table" columns={[
              { key: "name", header: t("codex.reset.hub.colName"), render: (incident) => incident.name },
              { key: "impact", header: t("codex.reset.hub.colImpact"), render: (incident) => impactLabel(incident.impact, t) },
              { key: "started", header: t("codex.reset.hub.colStarted"), render: (incident) => <Time iso={incident.startedAt} /> },
              { key: "resolved", header: t("codex.reset.hub.colResolved"), render: (incident) =>
                incident.resolvedAt !== null ? <Time iso={incident.resolvedAt} /> : t("codex.reset.hub.ongoing") },
            ]} />
        </CollapsibleSection>
      )}
    </section>
  );
}

export function CodexResetHubPanel({ read }: { read: HubRead }) {
  const { t } = useI18n();
  const { snapshot, cacheLoading, cacheError, readError } = read;
  const status = snapshot?.status ?? null;
  const cacheWarning = snapshot?.cacheWarning ?? null;
  // The native hub supplies timeline events in descending timestamp order.
  const latestEvent = status?.timeline.events[0];

  return (
    <>
      {cacheLoading && status === null && (
        <div className="asb-quota-loading" role="status" aria-label={t("codex.reset.loadingCache")}>
          <span className="asb-skeleton" aria-hidden="true" />
          <span className="asb-skeleton" aria-hidden="true" />
        </div>
      )}
      {status === null && !cacheLoading && readError === null && (
        <div className="asb-empty-state">
          <span className="asb-empty-state-icon" aria-hidden="true">
            <UpdateIcon />
          </span>
          <h4 className="asb-section-title">{t("codex.reset.empty")}</h4>
        </div>
      )}
      {cacheError && <p className="asb-warn-text" role="alert">{t("codex.reset.cacheUnavailable", { error: cacheError })}</p>}
      {readError && (
        <p className="asb-warn-text" role="alert">
          {t("codex.reset.readFailed", { error: readError })}
          {status !== null ? t("codex.reset.showingStale") : ""}
        </p>
      )}
      {status && (
        <>
          {status.sourceWarning && <p className="asb-warn-text">{status.sourceWarning}</p>}
          {cacheWarning && <p className="asb-warn-text">{cacheWarning}</p>}
          <div className="asb-codex-reset-hub">
            <HubSourceMeta status={status} cached={snapshot?.freshness === "cached"} />
            <div className="asb-codex-reset-board asb-codex-reset-hub-board">
              <HubActivity latestEvent={latestEvent} lastResetAt={status.forecast.lastResetAt}
                signal={status.feed.signal} />
              <HubForecastSummary forecast={status.forecast} />
            </div>
            <HubTimelineHistory timeline={status.timeline} />
            <HubBankedHistory observations={status.feed.banked} />
            <HubPlatformStatus platform={status.platform} />
          </div>
        </>
      )}
    </>
  );
}
