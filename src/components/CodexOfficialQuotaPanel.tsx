import { queryCodexOfficialQuota, type CodexOfficialQuota } from "../api/client";
import { Time } from "./Time";
import { QuotaWindowsTable } from "./QuotaWindowsTable";
import { useAutoQuery } from "./use-auto-query";

interface Props {
  id: string;
  profileId: string;
  profileName: string;
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

/** The official Codex quota has one native read-only path. It intentionally
 * does not consume provider usage-query settings, API keys, or endpoints. */
export function CodexOfficialQuotaPanel({ id, profileId, profileName }: Props) {
  /** The official read has no configurable query, so it stays manual-only:
   * read on mount, then refresh by hand. */
  const { data: reading, querying, error: requestError, run } = useAutoQuery(
    profileId,
    0,
    queryCodexOfficialQuota,
    "订阅额度读取失败",
  );

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
        <QuotaWindowsTable
          windows={reading!.windows}
          ariaLabel={`${profileName} 官方订阅额度`}
        />
      )}
      {!reading && !requestError && (
        <p className="asb-provider-usage-state" role="status">正在读取官方订阅额度…</p>
      )}
      {status && <p className="asb-warn-text" role="alert">{status}</p>}
      {requestError && <p className="asb-warn-text" role="alert">{requestError}</p>}
    </section>
  );
}
