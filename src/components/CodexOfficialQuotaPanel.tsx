import { useCallback, useEffect, useRef, useState } from "react";
import {
  queryCodexOfficialQuota,
  type CodexOfficialQuota,
  type CodexOfficialQuotaWindow,
} from "../api/client";
import { Time } from "./Time";

interface Props {
  id: string;
  profileId: string;
  profileName: string;
}

function percentage(value: number): string {
  return `${Math.round(value)}%`;
}

function statusCopy(quota: CodexOfficialQuota): string | null {
  switch (quota.status) {
    case "available":
      return null;
    case "signInRequired":
      return "未检测到可用的 Codex 官方登录。请完成登录后刷新。";
    case "reauthenticationRequired":
      return "Codex 官方登录已失效。请重新登录后刷新。";
    case "unavailable":
      return quota.stale
        ? "未能刷新，正在显示上次成功读取的额度。"
        : "暂时无法读取订阅额度，请稍后刷新。";
  }
}

function QuotaWindowLedger({ window }: { window: CodexOfficialQuotaWindow }) {
  return (
    <div className="asb-official-quota-window">
      <div className="asb-official-quota-window-line">
        <h4>{window.label}</h4>
        <span className="asb-official-quota-value">
          <strong>{percentage(window.usedPercent)}</strong>
          <span>已用</span>
          {window.resetsAt && (
            <small>
              重置 <Time iso={window.resetsAt} />
            </small>
          )}
        </span>
      </div>
      <div
        className="asb-provider-usage-progress"
        role="progressbar"
        aria-label={`${window.label} 已用比例`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(window.usedPercent)}
      >
        <span style={{ width: `${window.usedPercent}%` }} />
      </div>
    </div>
  );
}

/** The official Codex quota has one native read-only path. It intentionally
 * does not consume provider usage-query settings, API keys, or endpoints. */
export function CodexOfficialQuotaPanel({ id, profileId, profileName }: Props) {
  const [reading, setReading] = useState<CodexOfficialQuota | null>(null);
  const [querying, setQuerying] = useState(false);
  const [requestError, setRequestError] = useState<string | null>(null);
  const requestVersion = useRef(0);

  const run = useCallback(async () => {
    const version = ++requestVersion.current;
    setQuerying(true);
    setRequestError(null);
    try {
      const next = await queryCodexOfficialQuota(profileId);
      if (requestVersion.current === version) setReading(next);
    } catch (caught) {
      if (requestVersion.current === version) {
        setRequestError((caught as { message?: string }).message ?? "订阅额度读取失败");
      }
    } finally {
      if (requestVersion.current === version) setQuerying(false);
    }
  }, [profileId]);

  useEffect(() => {
    void run();
    return () => {
      requestVersion.current += 1;
    };
  }, [run]);

  const status = reading ? statusCopy(reading) : null;
  const showsWindows = (reading?.windows.length ?? 0) > 0;

  return (
    <section id={id} className="asb-official-quota" aria-label={`${profileName} 官方订阅额度`}>
      <header className="asb-provider-usage-head">
        <div className="asb-provider-usage-title">
          <h3>订阅额度</h3>
        </div>
        <div className="asb-provider-usage-actions">
          {reading?.at && <Time iso={reading.at} />}
          <button
            type="button"
            className="asb-provider-usage-refresh"
            disabled={querying}
            onClick={() => void run()}
          >
            {querying ? "读取中…" : "刷新"}
          </button>
        </div>
      </header>

      {showsWindows && (
        <div className="asb-official-quota-windows">
          {reading!.windows.map((window, index) => (
            <QuotaWindowLedger key={`${window.label}-${index}`} window={window} />
          ))}
        </div>
      )}
      {!reading && !requestError && (
        <p className="asb-provider-usage-state" role="status">正在读取官方订阅额度…</p>
      )}
      {status && <p className="asb-warn-text" role="alert">{status}</p>}
      {requestError && <p className="asb-warn-text" role="alert">{requestError}</p>}
    </section>
  );
}
