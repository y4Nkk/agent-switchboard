import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  checkCodexResetStatus,
  getCachedCodexResetStatus,
  type CodexResetRead,
  type ResetSignal,
} from "../api/client";
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

function ResetFact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <article className="asb-codex-reset-item" aria-label={label}>
      <h3 className="asb-codex-reset-label">{label}</h3>
      {children}
    </article>
  );
}

/** An explicit, read-only view of public global reset signals. */
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
          <button
            type="button"
            className="asb-btn-secondary"
            disabled={loading}
            onClick={() => void readStatus()}
          >
            {loading ? "读取中…" : "刷新重置信号"}
          </button>
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
          <div className="asb-codex-reset-grid">
            <ResetFact label="是否重置">
              {status.latestConfirmedReset ? (
                <>
                  <strong className="asb-codex-reset-value asb-ok-text">已确认全局重置</strong>
                  <p className="asb-codex-reset-detail">
                    信号时间 <Time iso={status.latestConfirmedReset.announcedAt} />
                  </p>
                </>
              ) : (
                <>
                  <strong className="asb-codex-reset-value">尚未发现完成信号</strong>
                  <p className="asb-codex-reset-detail">公开 feed 未提供已确认的全局重置。</p>
                </>
              )}
            </ResetFact>
            <ResetFact label="预计重置">
              {status.nextScheduledReset ? (
                <>
                  <strong className="asb-codex-reset-value">
                    <Time iso={signalTime(status.nextScheduledReset)} />
                  </strong>
                  <p className="asb-codex-reset-detail">
                    {scheduleDescription(status.nextScheduledReset)} · {confidenceLabel(status.nextScheduledReset.confidence)}
                  </p>
                </>
              ) : (
                <>
                  <strong className="asb-codex-reset-value">暂无公告预计</strong>
                  <p className="asb-codex-reset-detail">公开 feed 没有待完成的重置安排。</p>
                </>
              )}
            </ResetFact>
            <ResetFact label="Tibo 最近相关动态">
              {status.latestRelevantTiboPost ? (
                <>
                  <p className="asb-codex-reset-post">{status.latestRelevantTiboPost.text}</p>
                  <div className="asb-codex-reset-post-actions">
                    <span className="asb-codex-reset-detail">
                      <Time iso={status.latestRelevantTiboPost.announcedAt} />
                    </span>
                    <button
                      type="button"
                      className="asb-btn-secondary"
                      onClick={() => void openUrl(status.latestRelevantTiboPost!.url)}
                    >
                      查看原帖
                    </button>
                  </div>
                </>
              ) : (
                <>
                  <strong className="asb-codex-reset-value">暂无相关动态</strong>
                  <p className="asb-codex-reset-detail">公开 feed 尚未收录可打开的 Tibo 相关帖子。</p>
                </>
              )}
            </ResetFact>
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
