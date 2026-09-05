import type { ProviderProfile } from "../api/client";
import { ProviderUsageTrend } from "./ProviderUsageTrend";
import { Time } from "./Time";
import { UsageReadingsTable } from "./UsageReadingsTable";
import type { ProviderUsage } from "./use-provider-usage";

interface Props {
  id: string;
  profile: ProviderProfile;
  usage: ProviderUsage;
  onConfigure?: (profile: ProviderProfile) => void;
}

/** One configured provider's default-visible usage table. Configuration
 * still returns to the dedicated workspace through the supplied callback. */
export function ProviderUsagePanel({ id, profile, usage, onConfigure }: Props) {
  const { data: summary, querying, error, run, history } = usage;

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

      <ProviderUsageTrend
        providerName={profile.name}
        series={history.series}
        loading={history.loading}
        error={history.error}
      />
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
