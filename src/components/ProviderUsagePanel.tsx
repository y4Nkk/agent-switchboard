import { useCallback, useEffect, useRef, useState } from "react";
import {
  queryProfileUsage,
  type ProviderProfile,
  type UsageQuery,
  type UsageReading,
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

function usageProgress(reading: UsageReading): number | null {
  if (reading.total === null || !Number.isFinite(reading.total) || reading.total <= 0) return null;
  const used = reading.used ?? (reading.remaining === null ? null : reading.total - reading.remaining);
  if (used === null || !Number.isFinite(used)) return null;
  return Math.min(100, Math.max(0, (used / reading.total) * 100));
}

function UsageReadingLedger({ reading }: { reading: UsageReading }) {
  const primary = reading.remaining !== null
    ? { label: "余额", value: reading.remaining }
    : { label: "已用", value: reading.used };
  const progress = usageProgress(reading);
  const name = reading.planName?.trim() || "默认额度";
  return (
    <div className="asb-provider-usage-reading">
      <div className="asb-provider-usage-reading-line">
        <h4>{name}</h4>
        <div className="asb-provider-usage-reading-values">
          <span className="asb-provider-usage-primary">
            <span>{primary.label}</span>
            <strong>{usageValue(primary.value)}</strong>
            {primary.value !== null && reading.unit && <small>{reading.unit}</small>}
          </span>
          {reading.remaining !== null && reading.used !== null && (
            <span className="asb-provider-usage-secondary">已用 {usageValue(reading.used)}</span>
          )}
          <span className="asb-provider-usage-total">
            总量 {usageValue(reading.total)}
            {reading.total !== null && reading.unit ? ` ${reading.unit}` : ""}
          </span>
        </div>
      </div>
      {progress !== null && (
        <div
          className="asb-provider-usage-progress"
          role="progressbar"
          aria-label={`${name} 已用比例`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progress)}
        >
          <span style={{ width: `${progress}%` }} />
        </div>
      )}
    </div>
  );
}

/** One configured provider's default-visible usage ledger. Configuration
 * still returns to the dedicated workspace through the supplied callback. */
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
      const next = await queryProfileUsage(profile.id);
      if (requestVersion.current === version) setSummary(next);
    } catch (caught) {
      if (requestVersion.current === version) {
        setError((caught as { message?: string }).message ?? "用量查询失败");
      }
    } finally {
      if (requestVersion.current === version) setQuerying(false);
    }
  }, [profile.id]);

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
          <h3>用量</h3>
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
        <div className="asb-provider-usage-readings">
          {summary.readings.map((reading, index) => (
            <UsageReadingLedger key={`${reading.planName ?? "默认"}-${index}`} reading={reading} />
          ))}
        </div>
      ) : (
        !error && <p className="asb-provider-usage-state" role="status">正在读取已配置的用量…</p>
      )}
      {error && <p className="asb-warn-text" role="alert">{error}</p>}
    </section>
  );
}
