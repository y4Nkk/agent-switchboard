import { useCallback, useEffect, useRef, useState } from "react";
import { cancelClientUpdate, clientUpdateActive, prepareClientUpdate, readClientSoftware, readClientUpdate,
  refreshClientSoftware, selectClientSoftware, startClientUpdate, type ClientSoftwareSnapshot, type ClientUpdatePlan, type ClientUpdateTask } from "../../api/client-software";
import type { AppKind } from "../../api/shared";
import { useMessageState } from "../../i18n/use-message-state";
import { latestClientUpdate } from "./task-state";

export function useClientSoftware(active: boolean) {
  const [snapshot, setSnapshot] = useState<ClientSoftwareSnapshot | null>(null);
  const [task, setTask] = useState<ClientUpdateTask | null>(null);
  const [plan, setPlan] = useState<ClientUpdatePlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const [loading, setLoading] = useState(false);
  const pending = useRef(false);
  const generation = useRef(0);
  const reconciledTask = useRef<string | null>(null);
  const receiveTask = useCallback((next: ClientUpdateTask | null) => setTask((current) => latestClientUpdate(current, next)), []);
  const receive = useCallback((next: ClientSoftwareSnapshot) => {
    setSnapshot(next); receiveTask(next.task);
    if (next.task && !clientUpdateActive(next.task)) reconciledTask.current = next.task.id;
  }, [receiveTask]);
  const execute = useCallback(async (operation: () => Promise<void>) => {
    if (pending.current) return;
    pending.current = true; generation.current++; setLoading(false); setBusy(true); setError(null);
    try { await operation(); } catch (error) { setError(error); }
    finally { pending.current = false; setBusy(false); }
  }, [setError]);
  const refresh = useCallback((force = true) => execute(async () => {
    setPlan(null); receive(await refreshClientSoftware(force));
  }), [execute, receive]);
  useEffect(() => {
    if (!active) return;
    const request = ++generation.current;
    setLoading(true);
    const load = async () => {
      const local = await readClientSoftware();
      if (generation.current !== request) return;
      receive(local);
      if (!clientUpdateActive(local.task)) {
        const latest = await refreshClientSoftware(false);
        if (generation.current === request) receive(latest);
      }
    };
    void load().catch((error: unknown) => { if (generation.current === request) setError(error); })
      .finally(() => { if (generation.current === request) setLoading(false); });
    return () => { generation.current++; };
  }, [active, receive, setError]);
  useTaskPolling(active, task, receiveTask, setError);
  useEffect(() => {
    if (!active || busy || loading || !task || clientUpdateActive(task) || reconciledTask.current === task.id) return;
    reconciledTask.current = task.id;
    void refresh(false);
  }, [active, busy, loading, task, refresh]);
  const prepare = (id: string) => execute(async () => {
    if (snapshot) setPlan(await prepareClientUpdate(id, snapshot.revision));
  });
  const start = () => execute(async () => {
    if (plan) { receiveTask(await startClientUpdate(plan.id)); setPlan(null); }
  });
  const cancel = () => execute(async () => { if (task) receiveTask(await cancelClientUpdate(task.id)); });
  const select = (client: AppKind) => execute(async () => {
    setPlan(null); const next = await selectClientSoftware(client); if (next) receive(next);
  });
  return { snapshot, task, plan, busy: busy || loading, updating: clientUpdateActive(task), error, refresh, prepare, start, cancel, select, dismissPlan: () => setPlan(null) };
}

function useTaskPolling(active: boolean, task: ClientUpdateTask | null,
  receive: (task: ClientUpdateTask | null) => void,
  onError: (error: unknown) => void) {
  const running = clientUpdateActive(task);
  const id = task?.id;
  useEffect(() => {
    if (!active || !running) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const next = await readClientUpdate();
        if (disposed) return;
        receive(next);
        if (!clientUpdateActive(next)) return;
      } catch (error) { if (!disposed) onError(error); }
      if (!disposed) timer = setTimeout(() => void poll(), 1000);
    };
    timer = setTimeout(() => void poll(), 500);
    return () => { disposed = true; clearTimeout(timer); };
  }, [active, running, id, receive, onError]);
}
