import type { ReactNode } from "react";
import type {
  AppKind,
  ConfigFileStatus,
  LockStatus,
  MatchStatus,
  ProviderProfile,
  RouteState,
} from "../api/client";
import { ClientLogo } from "../components/ClientLogo";
import { CodexOfficialResetPanel } from "../components/CodexOfficialResetPanel";
import { CodexResetPanel } from "../components/CodexResetPanel";
import { DualRelay } from "../components/DualRelay";
import { RuntimeOverviewPanel } from "../components/RuntimeOverviewPanel";
import { Button } from "../components/Button";
import { Time } from "../components/Time";
import { clientName } from "../lib/client-name";
import { currentProviderName } from "../lib/current-provider-name";

interface OverviewPageProps {
  statuses: ConfigFileStatus[] | null;
  profiles: ProviderProfile[];
  locks: Partial<Record<AppKind, LockStatus>>;
  busy: boolean;
  /** The relay stays hidden while the provider editor is open. */
  relayHidden: boolean;
  onRefresh: () => void;
  onRecoverLock: (app: AppKind) => void;
}

function routesFrom(statuses: ConfigFileStatus[]): {
  codex: RouteState | null;
  claude: RouteState | null;
} {
  const find = (app: string) => statuses.find((status) => status.app === app)?.route ?? null;
  return { codex: find("codex"), claude: find("claude") };
}

function providerNamesFrom(
  statuses: ConfigFileStatus[],
  profiles: ProviderProfile[],
): Partial<Record<AppKind, string>> {
  return Object.fromEntries(
    statuses.map((status) => [status.app, currentProviderName(status, profiles)]),
  ) as Partial<Record<AppKind, string>>;
}

function matchLabel(status: MatchStatus): ReactNode {
  switch (status.kind) {
    case "matchesProfile":
      return `与档案「${status.profileName}」一致`;
    case "profileChanged":
      return `档案「${status.profileName}」或通用设置已变更，尚未应用`;
    case "restoredBackup":
      return (
        <>
          当前为已恢复备份（<Time iso={status.at} />）
        </>
      );
    case "externallyModified":
      return (
        <>
          与上次切换 (<Time iso={status.at} />) 不符，配置可能被外部修改
        </>
      );
    case "unmanaged":
      return "从未由本应用切换，也不匹配任何档案";
    case "unknown":
      return "无法评估（文件缺失或语法错误）";
  }
}

function lockLabel(status: LockStatus | undefined): string {
  if (!status) return "写入锁状态加载中";
  switch (status.state) {
    case "free":
      return "写入锁空闲";
    case "held": {
      const holder = status.processName ?? (status.pid ? `进程 ${status.pid}` : "其他进程");
      return `写入锁由${holder}持有`;
    }
    case "stale":
      return "发现遗留写入锁，可在确认后清理";
    case "indeterminate":
      return `写入锁状态无法确定：${status.reason}`;
  }
}

function statusPill(status: ConfigFileStatus): { ok: boolean; text: string } {
  if (status.readError) return { ok: false, text: "读取失败" };
  if (!status.exists) return { ok: false, text: "未找到配置文件" };
  if (!status.syntaxOk) return { ok: false, text: "语法错误" };
  return { ok: true, text: "配置正常" };
}

function ConfigStatusCard({
  status,
  profiles,
  lock,
  busy,
  onRecoverLock,
}: {
  status: ConfigFileStatus;
  profiles: ProviderProfile[];
  lock: LockStatus | undefined;
  busy: boolean;
  onRecoverLock: (app: AppKind) => void;
}) {
  const pill = statusPill(status);
  const readable = !status.readError && status.exists && status.syntaxOk;
  return (
    <article className="asb-status-card" aria-label={`${clientName(status.app)} 配置状态`}>
      <header className="asb-status-head">
        <h3 className="asb-status-name">
          <ClientLogo app={status.app} className="asb-status-logo" />
          {clientName(status.app)}
        </h3>
        <span className={`asb-status-pill${pill.ok ? " is-ok" : ""}`}>
          <span className="asb-status-pill-dot" aria-hidden="true" />
          {pill.text}
        </span>
      </header>
      <dl className="asb-status-rows">
        <div className="asb-status-row">
          <dt>配置文件</dt>
          <dd className="asb-code">{status.path}</dd>
        </div>
        {status.readError && (
          <div className="asb-status-row">
            <dt>读取错误</dt>
            <dd className="asb-warn-text">{status.readError}</dd>
          </div>
        )}
        {readable && (
          <div className="asb-status-row">
            <dt>当前服务</dt>
            <dd>
              {currentProviderName(status, profiles)}
              {" · "}
              {status.route?.model ?? "默认模型"}
            </dd>
          </div>
        )}
        {readable && (
          <div className="asb-status-row">
            <dt>匹配状态</dt>
            <dd>{matchLabel(status.matchStatus)}</dd>
          </div>
        )}
        {status.lastSwitch && (
          <div className="asb-status-row">
            <dt>上次切换</dt>
            <dd>
              <Time iso={status.lastSwitch.at} />
              {status.lastSwitch.profileName
                ? ` · ${status.lastSwitch.profileName}`
                : " · 已恢复备份"}
            </dd>
          </div>
        )}
        {(status.route?.scopeWarnings.length ?? 0) > 0 && (
          <div className="asb-status-row">
            <dt>范围警告</dt>
            <dd>
              {status.route?.scopeWarnings.map((warning) => (
                <span key={warning} className="asb-warn-text asb-status-warn">
                  {warning}
                </span>
              ))}
            </dd>
          </div>
        )}
        <div className="asb-status-row">
          <dt>写入锁</dt>
          <dd>{lockLabel(lock)}</dd>
        </div>
      </dl>
      {lock?.state === "stale" && (
        <div className="asb-kv-actions">
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => onRecoverLock(status.app)}
          >
            清理遗留锁
          </Button>
        </div>
      )}
    </article>
  );
}

/** Overview: the dual-client relay plus per-client configuration status. */
export function OverviewPage({
  statuses,
  profiles,
  locks,
  busy,
  relayHidden,
  onRefresh,
  onRecoverLock,
}: OverviewPageProps) {
  return (
    <>
      {!relayHidden && (
        <DualRelay
          routes={routesFrom(statuses ?? [])}
          providerNames={providerNamesFrom(statuses ?? [], profiles)}
        />
      )}
      <section className="asb-panel" aria-label="配置状态">
        <div className="asb-panel-heading">
          <h2 className="asb-panel-title">配置状态</h2>
          <Button variant="secondary" disabled={busy} onClick={onRefresh}>
            刷新状态
          </Button>
        </div>
        <div className="asb-status-grid">
          {(statuses ?? []).map((status) => (
            <ConfigStatusCard
              key={status.app}
              status={status}
              profiles={profiles}
              lock={locks[status.app]}
              busy={busy}
              onRecoverLock={onRecoverLock}
            />
          ))}
        </div>
        {statuses === null && <p className="asb-empty">加载中</p>}
      </section>
      <CodexResetPanel />
      <CodexOfficialResetPanel />
      <RuntimeOverviewPanel />
    </>
  );
}
