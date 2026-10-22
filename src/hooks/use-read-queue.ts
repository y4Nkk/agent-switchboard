import { useState } from "react";

type ReadJob = { run: () => Promise<void>; failed: (reason: unknown) => void };

export function createReadQueue() {
  let pending: ReadJob | null = null;
  let running = false;
  const drain = async () => {
    if (running) return;
    running = true;
    try {
      while (pending) {
        const job: ReadJob = pending;
        pending = null;
        try { await job.run(); }
        catch (reason) { job.failed(reason); }
      }
    } finally { running = false; }
  };
  return (run: ReadJob["run"], failed: ReadJob["failed"]) => {
    const job = { run, failed };
    pending = job;
    void drain();
    // Running native reads finish; cleanup releases only unstarted work.
    return () => { if (pending === job) pending = null; };
  };
}

export function useReadQueue() {
  const [enqueue] = useState(() => createReadQueue());
  return enqueue;
}
