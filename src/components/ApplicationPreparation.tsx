import { useEffect, useRef, useState } from "react";
import type { ApplicationPreparation as Snapshot } from "../api/application-preparation";
import { quitTray, openAppDataDir, openRuntimeLogDir, type CommandError } from "../api/client";
import { useI18n, type MessageKey } from "../i18n";
import { CommandErrorLines } from "../app/notifications";
import { isBrowserDevelopment } from "../lib/runtime";
import { Button } from "./Button";
import { WindowControls } from "./WindowControls";
import appIcon from "../assets/app-icon.svg";

const stages = {
  preferences: "preparation.stage.preferences", configuration: "preparation.stage.configuration",
  outboundProxy: "preparation.stage.outboundProxy", services: "preparation.stage.services", complete: "preparation.stage.complete",
} as const satisfies Record<Snapshot["stage"], MessageKey>;

function Elapsed({ snapshot }: { snapshot: Snapshot }) {
  const { t } = useI18n();
  const [elapsed, setElapsed] = useState(snapshot.elapsedMs);
  useEffect(() => {
    setElapsed(snapshot.elapsedMs);
    if (snapshot.status !== "running") return;
    const received = performance.now();
    const timer = window.setInterval(() => setElapsed(snapshot.elapsedMs + performance.now() - received), 1000);
    return () => window.clearInterval(timer);
  }, [snapshot.runId, snapshot.elapsedMs, snapshot.status]);
  const seconds = Math.floor(elapsed / 1000);
  return <p className="asb-preparation-elapsed">{t("preparation.elapsed", { minutes: Math.floor(seconds / 60), seconds: seconds % 60 })}</p>;
}

export function PreparationDetails({ snapshot }: { snapshot: Snapshot }) {
  const { t } = useI18n();
  return <div className="asb-preparation-details">
    {snapshot.issues.map((issue, index) => <div className="asb-preparation-issue" key={`${issue.code}-${index}`}>
      <CommandErrorLines error={issue} />
    </div>)}
    {snapshot.preferencesReset && <p>{t("preparation.preferencesReset")}</p>}
    {snapshot.repairedFiles.length > 0 && <p>{t("preparation.repaired", { count: snapshot.repairedFiles.length })}</p>}
    {snapshot.backupPaths.map((path) => <p key={path}>{t("preparation.backup", { path })}</p>)}
  </div>;
}

export function ApplicationPreparation({ snapshot, error, onRecheck, overlay = false }: {
  snapshot: Snapshot | null; error: CommandError | null; onRecheck: () => void; overlay?: boolean;
}) {
  const { t } = useI18n();
  const [actionError, setActionError] = useState<CommandError | null>(null);
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!overlay) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    content.current?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, [overlay]);
  const running = !error && (!snapshot || snapshot.status === "running");
  const action = (run: () => Promise<unknown>) => { void run().catch((caught) => setActionError(caught as CommandError)); };
  const Surface = overlay ? "div" : "main";
  return <Surface role={overlay ? "dialog" : undefined} aria-modal={overlay ? true : undefined} className={`asb-preparation${overlay ? " is-overlay" : ""}`} aria-label="Agent Switchboard">
    <header className="asb-preparation-titlebar" data-tauri-drag-region>
      <span data-tauri-drag-region>Agent Switchboard</span>
      {!isBrowserDevelopment && <WindowControls />}
    </header>
    <div ref={content} tabIndex={overlay ? -1 : undefined} className="asb-preparation-content">
      <img className="asb-preparation-logo" src={appIcon} alt="" />
      {!snapshot?.settings && running ? <div className="asb-settings-skeleton" role="status" aria-label="Agent Switchboard">
        <div className="asb-skeleton" /><div className="asb-skeleton" />
      </div> : <>
        <h1>{t(running ? "preparation.title" : "preparation.blocked")}</h1>
        <p className="asb-preparation-copy" role={running ? "status" : "alert"} aria-live="polite">
          {running ? t(snapshot ? stages[snapshot.stage] : "preparation.connecting") : t("preparation.blockedDetail")}
        </p>
      </>}
      {running && <div className="asb-preparation-progress" aria-hidden="true"><span /></div>}
      {snapshot && <Elapsed snapshot={snapshot} />}
      {error && <div role="alert"><CommandErrorLines error={error} /></div>}
      {!running && snapshot && <PreparationDetails snapshot={snapshot} />}
      {actionError && <div role="alert"><CommandErrorLines error={actionError} /></div>}
      <div className="asb-preparation-actions">
        {!running && <>
          <Button variant="secondary" onClick={onRecheck}>{t("preparation.recheck")}</Button>
          <Button variant="secondary" onClick={() => action(openRuntimeLogDir)}>{t("preparation.logs")}</Button>
          <Button variant="secondary" onClick={() => action(openAppDataDir)}>{t("preparation.originals")}</Button>
        </>}
        <Button variant="ghost" onClick={() => action(quitTray)}>{t("preparation.exit")}</Button>
      </div>
    </div>
  </Surface>;
}
