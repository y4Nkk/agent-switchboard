import { useMessageState } from "../../i18n/use-message-state";
import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n";
import { sourceErrorMessage } from "./skill-source-model";

export function useSkillSourceRequests<T>(busy: boolean) {
  const { t } = useI18n();
  const generation = useRef(0);
  const active = useRef<number | null>(null);
  const latest = useRef<T | null>(null);
  const [result, setResult] = useState<T | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useMessageState();
  useEffect(() => () => { generation.current += 1; active.current = null; }, []);

  const cancel = useCallback(() => {
    generation.current += 1;
    active.current = null;
    setLoading(false);
    setError(null);
  }, [setError]);
  const update = useCallback((transform: (previous: T | null) => T | null) => {
    latest.current = transform(latest.current);
    setResult(latest.current);
  }, []);
  const clear = useCallback(() => { cancel(); update(() => null); }, [cancel, update]);
  const run = async (read: (isCurrent: () => boolean) => Promise<T | null>) => {
    if (busy || active.current !== null) return null;
    const id = ++generation.current;
    active.current = id;
    const current = () => generation.current === id;
    setLoading(true);
    setError(null);
    try {
      const next = await read(current);
      if (!current()) return null;
      if (next !== null) update(() => next);
      return next;
    } catch (reason) {
      if (current()) setError(reason);
      return null;
    } finally {
      if (current()) { active.current = null; setLoading(false); }
    }
  };
  const detail = error ? sourceErrorMessage(error, t("extensions.sources.readFailedRetry")) : null;
  const message = detail && result !== null ? `${detail}${t("extensions.sources.staleResults")}` : detail;
  return { result, loading, error: message, setError, clear, cancel, update, run };
}
