import { useId, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  type CodexResetHeatmap,
  type CodexResetHeatmapDay,
  type ResetSignal,
  type ResetType,
} from "../api/client";
import type { MessageKey, TFunction } from "../i18n";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { CollapsibleSection } from "./Collapsible";
import { Time } from "./Time";
import { UpdateIcon } from "./icons";
import { ModuleHeader } from "./WorkspaceHeader";
import { CodexResetHubPanel } from "./CodexResetHubPanel";
import { type CodexResetSource, useCodexResetSignal } from "./quota-reads";
import { Table } from "./Table";
import { Tabs } from "./Tabs";
import { Tooltip } from "./Tooltip";

const DAY_MS = 86_400_000;

const RESET_TYPE_LABELS: Record<ResetType, MessageKey> = {
  global: "codex.reset.typeGlobal",
  banked: "codex.reset.typeBanked",
  other: "codex.reset.typeOther",
};

function resetTypeBadgeClass(type: ResetType): string {
  switch (type) {
    case "global":
      return " is-global";
    case "banked":
      return " is-banked";
    case "other":
      return "";
  }
}

function scheduleDescription(signal: ResetSignal, t: TFunction): string {
  if (signal.effectiveAt === null) return t("codex.reset.noTimeAnnounced");
  return signal.schedulePrecision === "date" ? t("codex.reset.datePrecision") : t("codex.reset.exactPrecision");
}

function confidenceLabel(confidence: number, t: TFunction): string {
  return t("codex.reset.confidence", { value: Math.round(confidence * 100) });
}

/** The site buckets a schedule's confidence down to the nearest ten percent:
 * 0.7 reads as "≥70%", and anything from 0.9 up reads as "≥90%". */
function probabilityLabel(confidence: number): string {
  return `≥${Math.min(9, Math.floor(confidence * 10)) * 10}%`;
}

interface ForecastWindow {
  start: string;
  end: string | null;
}

/** A date-level schedule is an arrival window (the announced day), so it
 * renders as start ~ end like the public site does; a datetime schedule is a
 * single point. */
function forecastWindow(signal: ResetSignal): ForecastWindow | null {
  if (signal.effectiveAt === null) return null;
  const end =
    signal.schedulePrecision === "date"
      ? new Date(new Date(signal.effectiveAt).getTime() + DAY_MS - 60_000).toISOString()
      : null;
  return { start: signal.effectiveAt, end };
}

/** Reset-category chip shared by both sources: global resets and banked
 * reset cards carry their site-tinted outline; generic categories stay neutral. */
export function ResetBadge({ type }: { type: ResetType }) {
  const { t } = useI18n();
  return (
    <span className={`asb-codex-reset-badge${resetTypeBadgeClass(type)}`}>
      {t(RESET_TYPE_LABELS[type])}
    </span>
  );
}

/** The feed's UTC-day history as a contribution-style grid. The grid anchors
 * on the newest known day (a scheduled future day included) and always spans
 * the feed's week count; days after the anchor stay invisible placeholders. */
function ResetHeatmap({ heatmap }: { heatmap: CodexResetHeatmap }) {
  const { t } = useI18n();
  const [dailyOpen, setDailyOpen] = useState(false);
  const levels = new Map(heatmap.days.map((day) => [day.date, day]));
  const todayKey = new Date().toISOString().slice(0, 10);
  const anchorKey = heatmap.days.reduce(
    (latest, day) => (day.date > latest ? day.date : latest),
    todayKey,
  );
  const anchorWeekday = new Date(`${anchorKey}T00:00:00Z`).getUTCDay();
  const gridEndTime = Date.parse(`${anchorKey}T00:00:00Z`) + (6 - anchorWeekday) * DAY_MS;
  const weeks = heatmap.weeks;
  const gridStartTime = gridEndTime - (weeks * 7 - 1) * DAY_MS;
  const cells: Array<{ key: string; day: CodexResetHeatmapDay | null; hidden: boolean }> = [];
  for (let column = 0; column < weeks; column += 1) {
    for (let row = 0; row < 7; row += 1) {
      const key = new Date(gridStartTime + (column * 7 + row) * DAY_MS).toISOString().slice(0, 10);
      cells.push({ key, day: levels.get(key) ?? null, hidden: key > anchorKey });
    }
  }

  return (
    <div
      className="asb-codex-reset-heatmap"
      role="group"
      aria-label={t("codex.reset.heatmapAria", { weeks, total: heatmap.total, timezone: heatmap.timezone })}
    >
      <h4 className="asb-group-title">{t("codex.reset.heatmapTitle")}</h4>
      <div className="asb-codex-reset-heatmap-grid" aria-hidden="true">
        {cells.map((cell) => {
          const className = `asb-codex-reset-heatmap-cell${cell.day ? ` is-level-${cell.day.level}` : ""}${cell.hidden ? " is-hidden" : ""}`;
          if (!cell.day) return <span key={cell.key} className={className} />;
          return <Tooltip key={cell.key} asChild label={t("codex.reset.heatmapCellTitle", { date: cell.key, count: cell.day.count })}>
            <span className={className} />
          </Tooltip>;
        })}
      </div>
      <div className="asb-codex-reset-heatmap-legend" aria-hidden="true">
        <span>{t("codex.reset.few")}</span>
        <span className="asb-codex-reset-heatmap-cell" />
        <span className="asb-codex-reset-heatmap-cell is-level-1" />
        <span className="asb-codex-reset-heatmap-cell is-level-2" />
        <span className="asb-codex-reset-heatmap-cell is-level-3" />
        <span className="asb-codex-reset-heatmap-cell is-level-4" />
        <span>{t("codex.reset.many")}</span>
      </div>
      <CollapsibleSection quiet open={dailyOpen} onOpenChange={setDailyOpen}
        label={t("codex.reset.dailyData")}>
        <p className="asb-codex-reset-detail">
          {t("codex.reset.heatmapTotal", { total: heatmap.total, weeks, timezone: heatmap.timezone })}
        </p>
        <Table rows={cells.filter((cell) => cell.day)} rowKey={(cell) => cell.key}
          ariaLabel={t("codex.reset.dailyData")} className="asb-codex-reset-daily-table" columns={[
            { key: "date", header: t("codex.reset.dailyDate"), render: (cell) => cell.key },
            { key: "count", header: t("codex.reset.dailyCount"), align: "end", render: (cell) => cell.day?.count ?? 0 },
          ]} />
      </CollapsibleSection>
    </div>
  );
}

type RunwayRead = ReturnType<typeof useCodexResetSignal>["runway"];
type RunwayStatus = NonNullable<RunwayRead["snapshot"]>["status"];

function RunwayForecast({ status }: { status: RunwayStatus }) {
  const { t } = useI18n();
  const forecast = status.nextScheduledReset;
  const window = forecast && forecastWindow(forecast);
  const post = status.latestRelevantTiboPost;
  return <article className="asb-codex-reset-summary" aria-label={t("codex.reset.forecastAria")}>
    <h4 className="asb-codex-reset-summary-title">{t("codex.reset.nextForecast")}</h4>
    {forecast ? <>
      <Tooltip asChild label={confidenceLabel(forecast.confidence, t)}>
        <strong className="asb-codex-reset-answer is-forecast" tabIndex={0}>{probabilityLabel(forecast.confidence)}</strong>
      </Tooltip>
      {window ? <p className="asb-codex-reset-detail">
        <Time iso={window.start} />{window.end && <> — <Time iso={window.end} /></>}
        {" · "}{scheduleDescription(forecast, t)}
      </p> : <p className="asb-codex-reset-detail">{scheduleDescription(forecast, t)}</p>}
    </> : <strong className="asb-codex-reset-answer is-none">{t("codex.reset.noAnnouncement")}</strong>}
    {post && <div className="asb-codex-reset-post-summary">
      <p className="asb-codex-reset-label">{t("codex.reset.tiboLabel")}</p>
      <p className="asb-codex-reset-post">{post.text}</p>
      <div className="asb-codex-reset-post-actions">
        <span className="asb-codex-reset-detail"><Time iso={post.announcedAt} /></span>
        <Button variant="secondary" size="compact" onClick={() => void openUrl(post.url)}>{t("codex.reset.viewPost")}</Button>
      </div>
    </div>}
  </article>;
}

function RunwayResetBody({ read }: { read: RunwayRead }) {
  const { t } = useI18n();
  const { snapshot, cacheLoading, cacheError, readError } = read;
  const status = snapshot?.status;
  const confirmed = status?.latestConfirmedSignal;
  const forecast = status?.nextScheduledReset;
  return <>
    {cacheError && <p className="asb-warn-text" role="alert">{t("codex.reset.cacheUnavailable", { error: cacheError })}</p>}
    {readError && <p className="asb-warn-text" role="alert">
      {t("codex.reset.readFailed", { error: readError })}{status ? t("codex.reset.showingStale") : ""}
    </p>}
    {cacheLoading && !status && <div className="asb-quota-loading" role="status" aria-label={t("codex.reset.loadingCache")}>
      <span className="asb-skeleton" aria-hidden="true" /><span className="asb-skeleton" aria-hidden="true" />
    </div>}
    {!status && !cacheLoading && !cacheError && !readError && <div className="asb-empty-state">
      <span className="asb-empty-state-icon" aria-hidden="true"><UpdateIcon /></span>
      <h4 className="asb-section-title">{t("codex.reset.empty")}</h4>
    </div>}
    {status && <>
      {status.sourceWarning && <p className="asb-warn-text">{status.sourceWarning}</p>}
      {snapshot?.cacheWarning && <p className="asb-warn-text">{snapshot.cacheWarning}</p>}
      <div className="asb-codex-reset-source asb-codex-reset-detail">
        <a className="asb-codex-reset-source-link" href={status.sourceUrl} onClick={(event) => {
          event.preventDefault();
          void openUrl(status.sourceUrl);
        }}>Codex Runway</a>
        <span>{t("codex.reset.feedGenerated")} <Time iso={status.generatedAt} /></span>
        <span>{t("codex.reset.lastSuccessCheck")} <Time iso={status.lastSuccessfulCheckAt} /></span>
        <span>{t(snapshot?.freshness === "cached" ? "codex.reset.cachedAt" : "codex.reset.readThisTime")} <Time iso={status.checkedAt} /></span>
      </div>
      <div className="asb-codex-reset-board">
        <RunwayForecast status={status} />
        <dl className="asb-codex-reset-facts" aria-label={t("codex.reset.factsAria")}>
          <div className="asb-codex-reset-fact">
            <dt>{t("codex.reset.lastCompleted")}</dt>
            <dd>{confirmed ? <>
              <ResetBadge type={confirmed.resetType} />
              <Time iso={confirmed.announcedAt} />
            </> : t("codex.reset.none")}</dd>
          </div>
          {forecast && <div className="asb-codex-reset-fact">
            <dt>{t("codex.reset.forecastType")}</dt>
            <dd><ResetBadge type={forecast.resetType} /></dd>
          </div>}
        </dl>
      </div>
      <ResetHeatmap heatmap={status.heatmap} />
    </>}
  </>;
}

export function CodexResetPanel({ read }: { read: ReturnType<typeof useCodexResetSignal> }) {
  const { t } = useI18n();
  const id = useId();
  const [source, setSource] = useState<CodexResetSource>("runway");
  return <section className="asb-usage-surface asb-codex-reset" aria-labelledby={`${id}-heading`}>
    <ModuleHeader id={`${id}-heading`} title={t("codex.reset.title")} primary={
      <Tabs value={source} onChange={setSource} scope={id} label={t("codex.reset.sourcesAria")}
        tabs={[
          { value: "runway", label: t("codex.reset.sourceRunway"), controls: `${id}-runway-panel` },
          { value: "hub", label: t("codex.reset.sourceHub"), controls: `${id}-hub-panel` },
        ]} />
    } primaryActions={<Button variant="secondary" size="compact" aria-busy={read.loading}
      disabled={read.loading} onClick={read.readStatus}>{t(read.loading ? "usage.quota.reading" : "usage.quota.refreshReset")}</Button>} />
    <div role="tabpanel" id={`${id}-runway-panel`} aria-labelledby={`${id}-runway-tab`} hidden={source !== "runway"}>
      {source === "runway" && <RunwayResetBody read={read.runway} />}
    </div>
    <div role="tabpanel" id={`${id}-hub-panel`} aria-labelledby={`${id}-hub-tab`} hidden={source !== "hub"}>
      {source === "hub" && <CodexResetHubPanel read={read.hub} />}
    </div>
  </section>;
}
