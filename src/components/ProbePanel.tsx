import { useEffect, useRef, useState } from "react";
import { probeEndpoint, type ProbeResult } from "../api/client";
import { Time } from "./Time";

interface Props {
  url: string | null;
}


/**
 * Manual endpoint verification: reachability, HTTP status, latency, and the
 * time of the probe. Informational only — nothing is selected automatically.
 */
export function ProbePanel({ url }: Props) {
  const [result, setResult] = useState<ProbeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestVersion = useRef(0);

  useEffect(() => {
    requestVersion.current += 1;
    setResult(null);
    setError(null);
    setBusy(false);
  }, [url]);

  if (!url) return null;

  const run = async () => {
    const version = requestVersion.current;
    setBusy(true);
    setError(null);
    try {
      const nextResult = await probeEndpoint(url);
      if (requestVersion.current === version) setResult(nextResult);
    } catch (caught) {
      if (requestVersion.current === version) {
        setError((caught as { message?: string }).message ?? "验证失败");
      }
    } finally {
      if (requestVersion.current === version) setBusy(false);
    }
  };

  return (
    <div className="asb-probe">
      <div className="asb-form-actions">
        <button type="button" className="asb-btn-secondary" disabled={busy} onClick={run}>
          验证端点
        </button>
      </div>
      {error && <p className="asb-warn-text">{error}</p>}
      {result && (
        <div className="asb-kv" aria-label="端点验证结果">
          <span
            className={`asb-kv-label ${result.reachable ? "asb-ok-text" : "asb-fail-text"}`}
          >
            {result.reachable
              ? `可达 · HTTP ${result.status ?? "?"} · ${result.latencyMs ?? "?"} 毫秒`
              : "不可达"}
          </span>
          <span className="asb-kv-value">
            {result.error ?? <Time iso={result.at} />}
          </span>
        </div>
      )}
    </div>
  );
}
