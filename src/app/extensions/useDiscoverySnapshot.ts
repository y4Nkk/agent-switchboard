import { useCallback, useRef, useState } from "react";
import { discoverExtensions, type CommandError, type ExtensionDiscovery } from "../../api/client";

export function useDiscoverySnapshot(onError: (error: CommandError) => void) {
  const [snapshot, setSnapshot] = useState<ExtensionDiscovery | null>(null);
  const [scanId, setScanId] = useState<string | undefined>(undefined);
  const [scanning, setScanning] = useState(false);
  const [stale, setStale] = useState(false);
  const started = useRef(false);
  const inFlight = useRef(false);
  const generation = useRef(0);

  const scan = useCallback(async () => {
    if (inFlight.current) return null;
    inFlight.current = true;
    const version = ++generation.current;
    setScanning(true);
    try {
      const next = await discoverExtensions();
      if (version === generation.current) { setSnapshot(next); setScanId(next.scanId); setStale(false); }
      return next;
    } catch (caught) {
      setStale(true);
      onError(caught as CommandError);
      return null;
    } finally {
      inFlight.current = false;
      setScanning(false);
    }
  }, [onError]);

  const release = useCallback(() => {
    if (inFlight.current) return;
    generation.current += 1;
    setSnapshot(null);
    setStale(false);
  }, []);

  const ensureInitialScan = useCallback(() => {
    if (started.current || snapshot !== null) return;
    started.current = true;
    void scan();
  }, [scan, snapshot]);

  return { snapshot, scanId, scanning, stale, scan, ensureInitialScan, release };
}
