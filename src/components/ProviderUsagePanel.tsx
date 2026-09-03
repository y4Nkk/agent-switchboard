import { queryProfileUsage, type ProviderProfile } from "../api/client";
import { Time } from "./Time";
import { UsageReadingsTable } from "./UsageReadingsTable";
import { useAutoQuery } from "./use-auto-query";

interface Props {
  id: string;
  profile: ProviderProfile;
  onConfigure?: (profile: ProviderProfile) => void;
}

/** One configured provider's default-visible usage table. Configuration
 * still returns to the dedicated workspace through the supplied callback. */
export function ProviderUsagePanel({ id, profile, onConfigure }: Props) {
  /** The auto-refresh cadence is owned by the profile's saved query; an
   * unconfigured profile (manual-only) never reaches this panel's timer. */
  const refreshIntervalMinutes = profile.usageQuery?.refreshIntervalMinutes ?? 0;
  const { data: summary, querying, error, run } = useAutoQuery(
    profile.id,
    refreshIntervalMinutes,
    queryProfileUsage,
    "用量查询失败",
  );

  return (
    <section id={id} className="asb-provider-usage" aria-label={`${profile.name} 用量`}>
      <header className="asb-provider-usage-head">
        <div className="asb-provider-usage-title">
          <h3>用量</h3>
        </div>
        <div className="asb-provider-usage-actions">
          {summary && <Time iso={summary.at} />}
          {onConfigure && (
            <button
              type="button"
              className="asb-provider-usage-configure"
              onClick={() => onConfigure(profile)}
            >
              编辑查询
            </button>
          )}
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

      {summary ? (
        <UsageReadingsTable
          readings={summary.readings}
          ariaLabel={`${profile.name} 用量读数`}
        />
      ) : (
        !error && <p className="asb-provider-usage-state" role="status">正在读取已配置的用量…</p>
      )}
      {error && <p className="asb-warn-text" role="alert">{error}</p>}
    </section>
  );
}
