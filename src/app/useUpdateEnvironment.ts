import { useCallback, useEffect, useRef, useState } from "react";
import { getRuntimeOverview, getUpdateChannel, type UpdateChannel } from "../api/client";

export function useUpdateEnvironment() {
  const [updateChannel, setUpdateChannel] = useState<UpdateChannel | null>(null);
  const [channelError, setChannelError] = useState<unknown>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const mounted = useRef(false);
  const reading = useRef(false);
  const readChannel = useCallback(async () => {
    if (reading.current) return;
    reading.current = true;
    setChannelError(null);
    try {
      const channel = await getUpdateChannel();
      if (mounted.current) setUpdateChannel(channel);
    } catch (error) { if (mounted.current) setChannelError(() => error); }
    finally { reading.current = false; }
  }, []);
  useEffect(() => {
    mounted.current = true;
    void readChannel();
    void getRuntimeOverview().then((overview) => {
      if (mounted.current) setAppVersion(overview.appVersion);
    }).catch(() => undefined);
    return () => { mounted.current = false; };
  }, [readChannel]);
  return { updateChannel, channelError, appVersion, readChannel };
}
