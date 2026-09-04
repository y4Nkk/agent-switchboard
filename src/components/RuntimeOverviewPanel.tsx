import { useEffect, useState } from "react";
import { getRuntimeOverview, type RuntimeOverview } from "../api/client";

function errorMessage(reason: unknown): string {
  return reason instanceof Error && reason.message ? reason.message : "未提供具体原因";
}

function buildModeLabel(buildMode: RuntimeOverview["buildMode"]): string {
  return buildMode === "release" ? "正式构建" : "调试构建";
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

function listenerLabel(transport: RuntimeOverview["transport"]): string {
  return transport.kind === "webDevelopment" ? `${transport.host}:${transport.port}` : "无 TCP 端口";
}

function responseLabel(transport: RuntimeOverview["transport"]): string {
  return transport.kind === "webDevelopment"
    ? `健康检查 HTTP ${transport.healthStatus}`
    : "桌面协议 · 已响应";
}

/** A compact, read-only footer for the application process itself. */
export function RuntimeOverviewPanel() {
  const [runtime, setRuntime] = useState<RuntimeOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    void getRuntimeOverview()
      .then((overview) => {
        if (active) setRuntime(overview);
      })
      .catch((reason) => {
        if (active) setError(errorMessage(reason));
      });

    return () => {
      active = false;
    };
  }, []);

  return (
    <section className="asb-panel asb-runtime-overview" aria-labelledby="runtime-overview-heading">
      <div className="asb-panel-heading">
        <h2 id="runtime-overview-heading" className="asb-panel-title">
          运行环境
        </h2>
      </div>
      {runtime === null && error === null && (
        <p className="asb-empty" role="status">
          正在读取运行环境…
        </p>
      )}
      {error && (
        <p className="asb-warn-text" role="alert">
          无法读取运行环境：{error}
        </p>
      )}
      {runtime !== null && (
        <dl className="asb-runtime-overview-grid">
          <div className="asb-runtime-overview-item">
            <dt>应用版本</dt>
            <dd>v{runtime.appVersion}</dd>
          </div>
          <div className="asb-runtime-overview-item">
            <dt>构建模式</dt>
            <dd>{buildModeLabel(runtime.buildMode)}</dd>
          </div>
          <div className="asb-runtime-overview-item">
            <dt>运行平台</dt>
            <dd>
              {platformLabel(runtime.platform)} · {architectureLabel(runtime.architecture)}
            </dd>
          </div>
          <div className="asb-runtime-overview-item">
            <dt>监听端口</dt>
            <dd className="asb-code">{listenerLabel(runtime.transport)}</dd>
          </div>
          <div className="asb-runtime-overview-item">
            <dt>响应情况</dt>
            <dd>{responseLabel(runtime.transport)}</dd>
          </div>
          <div className="asb-runtime-overview-item asb-runtime-overview-path">
            <dt>应用数据</dt>
            <dd className="asb-code">{runtime.appDataPath}</dd>
          </div>
        </dl>
      )}
    </section>
  );
}
