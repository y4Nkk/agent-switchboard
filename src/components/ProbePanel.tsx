import { useEffect, useRef, useState } from "react";
import { probeEndpoint, type ProbeResult } from "../api/client";
import { Button } from "./Button";
import { Time } from "./Time";

interface Props {
  url: string | null;
}

interface FeedbackProps {
  id?: string;
  className?: string;
  result: ProbeResult | null;
  error: string | null;
}

/** Shared request lifecycle for every visible endpoint-probe trigger. */
export function useEndpointProbe(url: string | null) {
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

  const run = async () => {
    if (!url || busy) return;
    const version = requestVersion.current;
    setBusy(true);
    setError(null);
    try {
      const nextResult = await probeEndpoint(url);
      if (requestVersion.current === version) setResult(nextResult);
    } catch (caught) {
      if (requestVersion.current === version) {
        setError((caught as { message?: string }).message ?? "检测失败");
      }
    } finally {
      if (requestVersion.current === version) setBusy(false);
    }
  };

  return { result, busy, error, run };
}

/** One result presentation for editor and supplier-card probe controls. */
export function ProbeFeedback({ id, className, result, error }: FeedbackProps) {
  if (!result && !error) return null;

  return (
    <div id={id} className={`asb-probe-feedback${className ? ` ${className}` : ""}`} aria-live="polite">
      {result && (
        <span
          className={`asb-kv-label ${
            result.grade === "ok"
              ? "asb-ok-text"
              : result.grade === "slow"
                ? "asb-warn-text"
                : "asb-fail-text"
          }`}
        >
          {result.grade === "unreachable"
            ? `无法连通 · ${result.error ?? "网络请求失败"}`
            : `${result.grade === "ok" ? "连通正常" : "连通但较慢"} · HTTP ${result.status ?? "?"} · ${result.latencyMs ?? "?"} 毫秒`}
          {" · "}
          <Time iso={result.at} />
        </span>
      )}
      {error ? (
        <p className="asb-warn-text" role="alert">{error}</p>
      ) : (
        result && (
          <p className="asb-scope-note">
            检测仅确认服务地址可达，不发送模型请求，也不验证密钥是否有效。
          </p>
        )
      )}
    </div>
  );
}

/** Manual reachability check for the endpoint being edited, ported from the
 * CC Switch pattern: any HTTP answer proves the address is reachable (graded
 * slow past the latency threshold), only network-level failures report as
 * unreachable. The probe sends no model request and carries no credential.
 * The button toggles: with the outcome on screen the next click collapses it,
 * and a collapsed click re-probes before unfolding again. */
export function ProbePanel({ url }: Props) {
  const probe = useEndpointProbe(url);
  const [open, setOpen] = useState(false);
  // A url change clears the outcome in the hook, so a stale open flag can
  // never show anything on its own.
  const visible = open && (probe.result !== null || probe.error !== null);

  if (!url) return null;

  return (
    <>
      <Button
        variant="secondary"
        aria-expanded={visible}
        aria-controls="editor-probe-feedback"
        disabled={probe.busy}
        onClick={() => {
          if (visible) {
            setOpen(false);
            return;
          }
          setOpen(true);
          void probe.run();
        }}
      >
        {probe.busy ? "检测中…" : visible ? "收起结果" : "检测连通"}
      </Button>
      {visible && (
        <ProbeFeedback
          id="editor-probe-feedback"
          className="asb-editor-probe-feedback"
          result={probe.result}
          error={probe.error}
        />
      )}
    </>
  );
}
