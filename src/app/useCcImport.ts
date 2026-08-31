import { useCallback, useState } from "react";
import {
  importCcswitchProfiles,
  scanCcswitch,
  type CcSwitchImportOutcome,
  type CcSwitchScan,
  type CommandError,
} from "../api/client";

interface CcImportDeps {
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  invalidateCandidates: () => void;
  refresh: () => Promise<void>;
}

/**
 * Read-only CC Switch scanning and selection import. Keys never cross this
 * boundary; only routing facts and drafts.
 */
export function useCcImport({
  busy,
  onError,
  clearError,
  setBusy,
  invalidateCandidates,
  refresh,
}: CcImportDeps) {
  const [ccScan, setCcScan] = useState<CcSwitchScan | null>(null);
  const [ccSelected, setCcSelected] = useState<Record<string, boolean>>({});
  const [ccResult, setCcResult] = useState<CcSwitchImportOutcome | null>(null);

  const runCcScan = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    clearError();
    try {
      const scan = await scanCcswitch();
      setCcScan(scan);
      setCcResult(null);
      // Fresh scan: select everything importable; exact duplicates stay off.
      const selection: Record<string, boolean> = {};
      for (const item of scan.providers) selection[item.key] = !item.existing;
      setCcSelected(selection);
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, clearError, onError, setBusy]);

  const runCcImport = useCallback(async () => {
    if (busy || !ccScan) return;
    const keys = ccScan.providers.filter((item) => ccSelected[item.key]).map((item) => item.key);
    if (keys.length === 0) return;
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      const result = await importCcswitchProfiles(keys);
      setCcResult(result);
      setCcScan(null);
      setCcSelected({});
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, ccScan, ccSelected, clearError, invalidateCandidates, onError, refresh, setBusy]);

  return { ccScan, ccSelected, setCcSelected, ccResult, runCcScan, runCcImport };
}
