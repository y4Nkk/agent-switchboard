import { useCallback, useEffect, useRef, useState } from "react";
import {
  testUsageQuery,
  type ProviderProfile,
  type UsageQuery,
  type UsageSummary,
} from "../api/client";
import { Time } from "./Time";

interface Props {
  id: string;
  profile: ProviderProfile;
  query: UsageQuery;
  onConfigure?: (profile: ProviderProfile) => void;
}

function usageValue(value: number | null): string {
  if (value === null) return "—";
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 }).format(value);
}

function UsageMetric({
  label,
  value,
  unit,
}: {
  label: string;
  value: number | null;
  unit: string | null;
}) {
  return (
    <div className="asb-provider-usage-metric">
      <dt>{label}</dt>
      <dd>
        <strong>{usageValue(value)}</strong>
        {value !== null && unit && <small>{unit}</small>}
      </dd>
    </div>
  );
}

/** One configured provider's on-demand usage readout. It only reads after the
 * user expands the provider row; configuration returns to the dedicated
 * workspace through the supplied callback. */
export function ProviderUsagePanel({ id, profile, query, onConfigure }: Props) {
  const [querying, setQuerying] = useState(false);
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestVersion = useRef(0);

  const run = useCallback(async () => {
    const version = ++requestVersion.current;
    setQuerying(true);
    setError(null);
    try {
      const next = await testUsageQuery(query, profile.apiKey, profile.baseUrl);
      if (requestVersion.current === version) setSummary(next);
    } catch (caught) {
      if (requestVersion.current === version) {
        setError((caught as { message?: string }).message ?? "用量查询失败");
      }
    } finally {
      if (requestVersion.current === version) setQuerying(false);
    }
  }, [profile.apiKey, profile.baseUrl, query]);

  useEffect(() => {
    void run();
    return () => {
      requestVersion.current += 1;
    };
  }, [run]);

  return (
    <section id={id} className="asb-provider-usage" aria-label={`${profile.name} 用量`}>
      <header className="asb-provider-usage-head">
        <div className="asb-provider-usage-title">
          <h3>用量读数</h3>
          <span>{query.kind === "script" ? "脚本" : "字段提取"}</span>
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
        <dl className="asb-provider-usage-ledger">
          <UsageMetric label="余额" value={summary.remaining} unit={summary.unit} />
          <UsageMetric label="已用" value={summary.used} unit={summary.unit} />
          <UsageMetric label="总量" value={summary.total} unit={summary.unit} />
        </dl>
      ) : (
        !error && <p className="asb-provider-usage-state" role="status">正在读取已配置的用量…</p>
      )}
      {error && <p className="asb-warn-text" role="alert">{error}</p>}
    </section>
  );
}
