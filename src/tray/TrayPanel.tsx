import { useEffect, useRef } from "react";
import { ArrowUpRight, LogOut } from "lucide-react";
import { openTrayMain, quitTray, type TraySnapshot } from "../api/client";
import { AppLogo } from "../components/AppLogo";
import { Button } from "@/components/Button";
import { ClientLogo } from "@/components/ClientLogo";
import { applyAppAppearance } from "@/lib/app-appearance";
import { useI18n } from "@/i18n";
import { TrayProviderItem } from "./TrayProviderItem";
import { useTraySnapshot } from "./useTraySnapshot";
import { useTrayActions } from "./useTrayActions";
import { useTrayPanelLayout } from "./useTrayPanelLayout";

type TrayApp = "codex" | "claude";

interface TrayGroupProps {
  app: TrayApp;
  providers: TraySnapshot["providers"];
  loaded: boolean;
  busy: boolean;
  switchingId: string | null;
  refreshingId: string | null;
  onSwitch: (providerId: string) => void;
  onRefreshUsage: (provider: TraySnapshot["providers"][number]) => void;
}

function TrayGroup({ app, providers, loaded, busy, switchingId, refreshingId, onSwitch, onRefreshUsage }: TrayGroupProps) {
  const label = app === "codex" ? "Codex" : "Claude Code";
  const { t } = useI18n();
  return (
    <section aria-label={label} className="tray-group">
      <h2 className="tray-group-heading"><ClientLogo app={app} className="tray-client-logo" />{label}</h2>
      {loaded && providers.length === 0 && <p className="tray-group-empty">{t("tray.emptyGroup")}</p>}
      {providers.map((provider) => (
        <TrayProviderItem
          key={provider.id}
          provider={provider}
          busy={busy}
          switchingId={switchingId}
          refreshingId={refreshingId}
          onSwitch={() => onSwitch(provider.id)}
          onRefreshUsage={() => onRefreshUsage(provider)}
        />
      ))}
    </section>
  );
}

function TrayPanelContent({ snapshot, readError, initialized, refresh }: {
  snapshot: TraySnapshot | null;
  readError: string | null;
  initialized: boolean;
  refresh: () => Promise<void>;
}) {
  const { t } = useI18n();
  const { error: actionError, setError, pending, run, switchProvider, refreshUsage } = useTrayActions(refresh);
  const panel = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const busy = pending !== null || snapshot?.switching === true;
  const error = actionError ?? readError ?? snapshot?.error ?? null;
  const providers = snapshot?.providers ?? [];
  const switchingId = pending?.kind === "switch" ? pending.providerId : null;
  const refreshingId = pending?.kind === "refresh" ? pending.providerId : null;
  useTrayPanelLayout(panel, list, initialized, error, setError);
  useEffect(() => { applyAppAppearance(snapshot?.settings ?? null); }, [snapshot?.settings]);
  return (
    <div ref={panel} className="tray-panel" aria-label={t("tray.panel.aria")}>
      <header className="tray-header">
        <AppLogo className="tray-app-logo" />
        <span className="tray-app-name">Agent Switchboard</span>
      </header>
      {error && <div role="alert" className="tray-error">{error}
        <Button variant="secondary" disabled={busy} onClick={() => void run({ kind: "panel" }, refresh)}>{t("tray.retry")}</Button></div>}
      <div ref={list} className="tray-list">
        <div>
          {!snapshot && !readError && <p role="status" className="tray-loading">{t("tray.loading")}</p>}
          {(["codex", "claude"] as const).map((app) => (
            <TrayGroup
              key={app}
              app={app}
              providers={providers.filter((provider) => provider.app === app)}
              loaded={snapshot !== null}
              busy={busy}
              switchingId={switchingId}
              refreshingId={refreshingId}
              onSwitch={switchProvider}
              onRefreshUsage={refreshUsage}
            />
          ))}
        </div>
      </div>
      <footer className="tray-footer">
        <Button variant="unstyled" className="tray-ghost-button" disabled={busy} onClick={() => void run({ kind: "panel" }, openTrayMain)}>
          <ArrowUpRight size={16} aria-hidden="true" />
          {t("tray.openMain")}
        </Button>
        <Button variant="unstyled" className="tray-ghost-button" disabled={busy} onClick={() => void run({ kind: "panel" }, quitTray)}><LogOut size={16} aria-hidden="true" />{t("tray.quit")}</Button>
      </footer>
    </div>
  );
}

/** Snapshot loading commits the language before publishing the first content. */
export function TrayPanel() {
  const { snapshot, error: readError, initialized, refresh } = useTraySnapshot();
  return <TrayPanelContent snapshot={snapshot} readError={readError} initialized={initialized} refresh={refresh} />;
}
