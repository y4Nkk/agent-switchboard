import { useEffect, useRef, useState } from "react";
import {
  getCachedCodexOfficialReset,
  refreshCodexOfficialReset,
  type CodexOfficialQuota,
  type CodexOfficialQuotaReset,
  type CodexOfficialQuotaStatus,
} from "../api/client";
import { Button } from "./Button";
import { OfficialQuotaTrend } from "./OfficialQuotaTrend";
import { Time } from "./Time";
import { QuotaWindowsTable } from "./QuotaWindowsTable";
import { useUsageHistory } from "./use-usage-history";

function errorMessage(reason: unknown): string {
  return reason instanceof Error && reason.message ? reason.message : "未提供具体原因";
}

function statusCopy(status: Exclude<CodexOfficialQuotaStatus, "available">): string {
  switch (status) {
    case "signInRequired":
      return "未检测到可用的 Codex 官方登录。请完成登录后刷新。";
    case "reauthenticationRequired":
      return "Codex 官方登录已失效。请重新登录后刷新。";
    case "unavailable":
      return "暂时无法读取官方额度，请稍后刷新。";
  }
}

function resetKindLabel(kind: CodexOfficialQuotaReset["kind"]): string {
  return kind === "early" ? "提前重置" : "例行重置";
}

/** An explicit, read-only view of the machine's own Codex official quota
 * reset state. It never reads the public reset-signal feed above it. */
export function CodexOfficialResetPanel() {
  const history = useUsageHistory({ kind: "official" });
  const [snapshot, setSnapshot] = useState<CodexOfficialQuota | null>(null);
  const [freshness, setFreshness] = useState<"cached" | "live">("cached");
  const [cacheLoading, setCacheLoading] = useState(true);
  const [cacheError, setCacheError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [readError, setReadError] = useState<string | null>(null);
  const [statusNotice, setStatusNotice] = useState<string | null>(null);
  const requestRef = useRef<Promise<CodexOfficialQuota> | null>(null);
  const cacheRevisionRef = useRef(0);

  useEffect(() => {
    let active = true;
    const revision = ++cacheRevisionRef.current;

    const loadCache = async () => {
      try {
        const cached = await getCachedCodexOfficialReset();
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

  // Only an available read is displayable; every other shape (including a
  // null cache) renders the empty state instead of dereferencing windows.
  const quota = snapshot !== null && snapshot.status === "available" ? snapshot : null;

  const readStatus = async () => {
    if (requestRef.current !== null) return;

    cacheRevisionRef.current += 1;
    setCacheLoading(false);
    setCacheError(null);
    setLoading(true);
    setReadError(null);
    setStatusNotice(null);
    const request = Promise.resolve().then(() => refreshCodexOfficialReset());
    requestRef.current = request;

    try {
      const next = await request;
      if (next.status === "available") {
        setSnapshot(next);
        setFreshness("live");
        void history.refresh();
      } else {
        setStatusNotice(statusCopy(next.status));
      }
    } catch (reason) {
      setReadError(errorMessage(reason));
    } finally {
      if (requestRef.current === request) requestRef.current = null;
      setLoading(false);
    }
  };

  return (
    <section className="asb-panel asb-official-reset" aria-labelledby="codex-official-reset-heading">
      <div className="asb-panel-heading">
        <h2 id="codex-official-reset-heading" className="asb-panel-title">
          Codex 官方额度重置
        </h2>
        <div className="asb-panel-actions">
          {quota !== null && (
            <span
              className={`asb-codex-reset-read-state is-${freshness}`}
              aria-live="polite"
            >
              {freshness === "cached" ? "本地缓存" : "刚刚刷新"}
            </span>
          )}
          <Button
            variant="secondary"
            disabled={loading}
            onClick={() => void readStatus()}
          >
            {loading ? "读取中…" : "刷新官方额度"}
          </Button>
        </div>
      </div>
      {cacheLoading && quota === null && (
        <p className="asb-empty" role="status">正在读取本地缓存</p>
      )}
      {quota === null && !cacheLoading && !cacheError && !statusNotice && !readError && (
        <p className="asb-empty">尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。</p>
      )}
      {cacheError && <p className="asb-warn-text" role="alert">本地缓存不可用：{cacheError}</p>}
      {statusNotice && <p className="asb-warn-text" role="alert">{statusNotice}</p>}
      {readError && (
        <p className="asb-warn-text" role="alert">
          无法刷新官方额度：{readError}
          {quota !== null ? "；仍在显示上次成功读取的数据。" : ""}
        </p>
      )}
      {quota !== null && (
        <>
          <OfficialQuotaTrend
            series={history.series}
            loading={history.loading}
            error={history.error}
            ariaLabel="Codex 官方额度趋势"
          />
          <QuotaWindowsTable windows={quota.windows} ariaLabel="Codex 官方额度窗口" />
          <p className="asb-official-reset-meta">
            {quota.lastReset ? (
              <>
                上次检测到重置：{resetKindLabel(quota.lastReset.kind)} · 信号时间{" "}
                <Time iso={quota.lastReset.observedAt} />
                {quota.lastReset.resetsAt && (
                  <>
                    {" · 新重置时间 "}
                    <Time iso={quota.lastReset.resetsAt} />
                  </>
                )}
              </>
            ) : (
              "尚未检测到重置（每次成功读取自动比对）"
            )}
          </p>
          {quota.at && (
            <p className="asb-official-reset-meta">
              {freshness === "cached" ? "缓存于" : "读取于"} <Time iso={quota.at} />
            </p>
          )}
          <p className="asb-official-reset-note">
            数据来自本机 Codex 官方登录，仅代表当前账号额度，与上方公开重置信号相互独立。
          </p>
        </>
      )}
    </section>
  );
}
