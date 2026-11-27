import { FolderOpen, Monitor, Network, Plug, Tag, Wrench } from "lucide-react";
import { useMessageState } from "../i18n/use-message-state";
import { useEffect, useState } from "react";
import { getRuntimeOverview, openAppDataDir, type RuntimeOverview } from "../api/client";
import { useI18n, type TFunction } from "../i18n";
import { FactList } from "./FactList";
import { FactPath } from "./FactPath";
import { ModuleHeader } from "./WorkspaceHeader";
import { Button } from "./Button";

function buildModeLabel(buildMode: RuntimeOverview["buildMode"], t: TFunction): string {
  return buildMode === "release" ? t("providers.runtime.build.release") : t("providers.runtime.build.debug");
}

function platformLabel(platform: string): string {
  switch (platform) {
    case "windows":
      return "Windows";
    case "macos":
      return "macOS";
    case "linux":
      return "Linux";
    default:
      return platform;
  }
}

function architectureLabel(architecture: string): string {
  switch (architecture) {
    case "x86_64":
      return "x64";
    case "aarch64":
      return "ARM64";
    default:
      return architecture;
  }
}

function listenerLabel(transport: RuntimeOverview["transport"], t: TFunction): string {
  return transport.kind === "webDevelopment" ? `${transport.host}:${transport.port}` : t("providers.runtime.noTcpPort");
}

function responseLabel(transport: RuntimeOverview["transport"], t: TFunction): string {
  return transport.kind === "webDevelopment"
    ? t("providers.runtime.health", { status: transport.healthStatus })
    : t("providers.runtime.desktop");
}

/** A compact, read-only footer for the application process itself. */
export function RuntimeOverviewPanel() {
  const { t } = useI18n();
  const [runtime, setRuntime] = useState<RuntimeOverview | null>(null);
  const [error, setError] = useMessageState();
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let active = true;
    setError(null);

    void getRuntimeOverview()
      .then((overview) => {
        if (active) setRuntime(overview);
      })
      .catch((reason) => {
        if (active) setError(reason);
      });

    return () => {
      active = false;
    };
  }, [revision]);

  return (
    <section className="asb-panel" aria-labelledby="runtime-overview-heading">
      <ModuleHeader id="runtime-overview-heading" title={t("providers.runtime.title")} />
      {runtime === null && error === null && (
        /* Six fact lines at the fact list's footprint, never a one-line note. */
        <div className="asb-runtime-overview-skeleton" role="status" aria-label={t("providers.runtime.loading")}>
          {Array.from({ length: 6 }, (_, line) => (
            <span key={line} className="asb-skeleton" aria-hidden="true" />
          ))}
        </div>
      )}
      {error && (
        <p className="asb-warn-text" role="alert">
          {t("providers.runtime.loadFailed", { detail: error })}
          {" "}<Button variant="secondary" onClick={() => setRevision((value) => value + 1)}>{t("settings.retry")}</Button>
        </p>
      )}
      {runtime !== null && (
        <FactList rows={[
          { label: t("providers.runtime.appVersion"), icon: Tag, format: "code", value: `v${runtime.appVersion}` },
          { label: t("providers.runtime.buildMode"), icon: Wrench, value: buildModeLabel(runtime.buildMode, t) },
          { label: t("providers.runtime.platform"), icon: Monitor,
            value: `${platformLabel(runtime.platform)} · ${architectureLabel(runtime.architecture)}` },
          { label: t("providers.runtime.listener"), icon: Plug, format: "code", value: listenerLabel(runtime.transport, t) },
          { label: t("providers.runtime.response"), icon: Network, value: responseLabel(runtime.transport, t) },
          { label: t("providers.runtime.appData"), icon: FolderOpen,
            value: <FactPath path={runtime.appDataPath} open={openAppDataDir} /> },
        ]} />
      )}
    </section>
  );
}
