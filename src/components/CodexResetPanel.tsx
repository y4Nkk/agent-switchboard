import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import {
  checkCodexResetStatus,
  getCachedCodexResetStatus,
  type CodexResetRead,
  type ResetSignal,
} from "../api/client";
import { Button } from "./Button";
import { Time } from "./Time";

function errorMessage(reason: unknown): string {
  return reason instanceof Error && reason.message ? reason.message : "未提供具体原因";
}

function signalTime(signal: ResetSignal): string {
  return signal.effectiveAt ?? signal.announcedAt;
}

function scheduleDescription(signal: ResetSignal): string {
  if (signal.effectiveAt === null) return "已公告，但未提供预计时间";
  return signal.schedulePrecision === "date" ? "日期级预告" : "精确时间预告";
}

function confidenceLabel(confidence: number): string {
  return `信心 ${Math.round(confidence * 100)}%`;
}

function resetDescription(signal: ResetSignal): string {
  switch (signal.resetType) {
    case "global":
      return "已确认全局重置";
    case "banked":
      return "已确认重置卡发放";
    case "other":
      return "已确认完成信号";
  }
}

function isSignalOnFeedDay(signal: ResetSignal | null, generatedAt: string): boolean {
  if (signal === null) return false;
  const signalDate = new Date(signalTime(signal));
  const feedDate = new Date(generatedAt);
  return [
    signalDate.getFullYear() === feedDate.getFullYear(),
    signalDate.getMonth() === feedDate.getMonth(),
    signalDate.getDate() === feedDate.getDate(),
  ].every(Boolean);
}

/** An explicit, read-only view of public reset signals. */
export function CodexResetPanel() {
  const [snapshot, setSnapshot] = useState<CodexResetRead | null>(null);
  const [cacheLoading, setCacheLoading] = useState(true);
  const [cacheError, setCacheError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [readError, setReadError] = useState<string | null>(null);
  const requestRef = useRef<Promise<CodexResetRead> | null>(null);
  const cacheRevisionRef = useRef(0);

  useEffect(() => {
    let active = true;
    const revision = ++cacheRevisionRef.current;

    const loadCache = async () => {
      try {
        const cached = await getCachedCodexResetStatus();
        if (active && cacheRevisionRef.current === revision) setSnapshot(cached);
      } catch (reason) {
        if (active && cacheRevisionRef.current === revision) setCacheError(errorMessage(reason));
      } finally {
        if (active && cacheRevisionRef.current === revision) setCacheLoading(false);
      }
    };

    void loadCache();
    return () => {
      active = false;
    };
  }, []);

  const readStatus = async () => {
    if (requestRef.current !== null) return;

    cacheRevisionRef.current += 1;
    setCacheLoading(false);
    setCacheError(null);
    setLoading(true);
    setReadError(null);
    const request = Promise.resolve().then(() => checkCodexResetStatus());
    requestRef.current = request;

    try {
      setSnapshot(await request);
    } catch (reason) {
      setReadError(errorMessage(reason));
    } finally {
      if (requestRef.current === request) requestRef.current = null;
      setLoading(false);
    }
  };

  const status = snapshot?.status ?? null;
  const hasSignalOnFeedDay = status !== null && isSignalOnFeedDay(status.latestConfirmedSignal, status.generatedAt);

  return (
    <section className="asb-panel asb-codex-reset" aria-labelledby="codex-reset-heading">
      <div className="asb-panel-heading">
        <h2 id="codex-reset-heading" className="asb-panel-title">
          Codex 重置信号
        </h2>
        <div className="asb-panel-actions">
          {snapshot !== null && (
            <span
              className={`asb-codex-reset-read-state is-${snapshot.freshness}`}
              aria-live="polite"
            >
              {snapshot.freshness === "cached" ? "本地缓存" : "刚刚刷新"}
            </span>
          )}
          <Button
            variant="secondary"
            disabled={loading}
            onClick={() => void readStatus()}
          >
            {loading ? "读取中…" : "刷新重置信号"}
          </Button>
        </div>
      </div>
      {cacheLoading && status === null && <p className="asb-empty" role="status">正在读取本地缓存</p>}
      {status === null && !cacheLoading && readError === null && (
        <p className="asb-empty">尚无本地缓存。手动刷新以读取公开重置信号。</p>
      )}
      {cacheError && <p className="asb-warn-text" role="alert">本地缓存不可用：{cacheError}</p>}
      {readError && (
        <p className="asb-warn-text" role="alert">
          无法刷新公开重置信号：{readError}
          {status !== null ? "；仍在显示上次成功读取的数据。" : ""}
        </p>
      )}
      {snapshot !== null && status !== null && (
        <>
          <div className="asb-codex-reset-board">
            <article className="asb-codex-reset-summary" aria-label="本次公开检查结果">
              <p className="asb-codex-reset-question">本次公开检查当日有重置信号吗？</p>
              <strong
                className={`asb-codex-reset-answer${hasSignalOnFeedDay ? " is-confirmed" : ""}`}
              >
                {hasSignalOnFeedDay ? "是" : "否"}
              </strong>
              {status.latestConfirmedSignal ? (
                <>
                  <p className="asb-codex-reset-summary-title">
                    {resetDescription(status.latestConfirmedSignal)}
                  </p>
                  <p className="asb-codex-reset-detail">
                    公开源确认时间 <Time iso={signalTime(status.latestConfirmedSignal)} />
                  </p>
                </>
              ) : (
                <p className="asb-codex-reset-detail">公开 feed 尚未提供已确认的完成信号。</p>
              )}
              {status.latestRelevantTiboPost && (
                <div className="asb-codex-reset-post-summary">
                  <p className="asb-codex-reset-label">Tibo 最近相关动态</p>
                  <p className="asb-codex-reset-post">{status.latestRelevantTiboPost.text}</p>
                  <div className="asb-codex-reset-post-actions">
                    <span className="asb-codex-reset-detail">
                      <Time iso={status.latestRelevantTiboPost.announcedAt} />
                    </span>
                    <Button
                      variant="secondary"
                      onClick={() => void openUrl(status.latestRelevantTiboPost!.url)}
                    >
                      查看原帖
                    </Button>
                  </div>
                </div>
              )}
            </article>
            <dl className="asb-codex-reset-facts" aria-label="公开信号详情">
              <div className="asb-codex-reset-fact">
                <dt>最近完成信号</dt>
                <dd>
                  {status.latestConfirmedSignal ? (
                    <>
                      <span>{resetDescription(status.latestConfirmedSignal)}</span>
                      <Time iso={signalTime(status.latestConfirmedSignal)} />
                    </>
                  ) : (
                    "暂无"
                  )}
                </dd>
              </div>
              <div className="asb-codex-reset-fact">
                <dt>最近检查</dt>
                <dd><Time iso={status.lastSuccessfulCheckAt} /></dd>
              </div>
              <div className="asb-codex-reset-fact">
                <dt>预计下次重置</dt>
                <dd>
                  {status.nextScheduledReset ? (
                    <>
                      <Time iso={signalTime(status.nextScheduledReset)} />
                      <span>
                        {scheduleDescription(status.nextScheduledReset)} · {confidenceLabel(status.nextScheduledReset.confidence)}
                      </span>
                    </>
                  ) : (
                    "暂无公告预计"
                  )}
                </dd>
              </div>
            </dl>
          </div>
          {status.sourceWarning && <p className="asb-warn-text">{status.sourceWarning}</p>}
          {snapshot.cacheWarning && <p className="asb-warn-text">{snapshot.cacheWarning}</p>}
          <p className="asb-codex-reset-source">
            公开 feed 生成于 <Time iso={status.generatedAt} /> · 最近成功检查 <Time iso={status.lastSuccessfulCheckAt} /> · {snapshot.freshness === "cached" ? "缓存于" : "本次读取"} <Time iso={status.checkedAt} />
          </p>
          <p className="asb-codex-reset-note">
            数据来自 Codex Runway 公开 feed：{" "}
            <a
              className="asb-codex-reset-source-link"
              href={status.sourceUrl}
              onClick={(event) => {
                event.preventDefault();
                void openUrl(status.sourceUrl);
              }}
            >
              {status.sourceUrl}
            </a>
            ，非 OpenAI 官方，不代表你的账号额度。
          </p>
        </>
      )}
    </section>
  );
}
