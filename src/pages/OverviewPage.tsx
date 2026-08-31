import { useRef, useState, type ReactNode } from "react";
import {
  listSessions,
  type AppKind,
  type ConfigFileStatus,
  type LockStatus,
  type MatchStatus,
  type ProviderProfile,
  type RouteState,
  type SessionScan,
} from "../api/client";
import { ClientLogo } from "../components/ClientLogo";
import { DualRelay } from "../components/DualRelay";
import { Time } from "../components/Time";
import { clientName } from "../lib/client-name";

interface OverviewPageProps {
  statuses: ConfigFileStatus[] | null;
  locks: Partial<Record<AppKind, LockStatus>>;
  selectedProfile: ProviderProfile | null;
  canSwitch: boolean;
  busy: boolean;
  /** The relay stays hidden while the provider editor is open. */
  relayHidden: boolean;
  onPreview: () => void;
  onRequestSwitch: () => void;
  onRefresh: () => void;
  onRecoverLock: (app: AppKind) => void;
  onOpenSessions: () => void;
}

function routesFrom(statuses: ConfigFileStatus[]): {
  codex: RouteState | null;
  claude: RouteState | null;
} {
  const find = (app: string) => statuses.find((status) => status.app === app)?.route ?? null;
  return { codex: find("codex"), claude: find("claude") };
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
    case "matchesSettings":
      return (
        <>
          与上次通用设置写入一致（<Time iso={status.at} />）
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
  lock,
  busy,
  onRecoverLock,
}: {
  status: ConfigFileStatus;
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
              {status.route?.routeMode === "official" ? "官方登录" : "自定义服务"}
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
          <button
            type="button"
            className="asb-btn-secondary"
            disabled={busy}
            onClick={() => onRecoverLock(status.app)}
          >
            清理遗留锁
          </button>
        </div>
      )}
    </article>
  );
}

const SESSION_APPS: readonly AppKind[] = ["codex", "claude"];

function latestSessionActivity(scan: SessionScan, app: AppKind): string | null {
  let latest: string | null = null;
  let latestTimestamp = Number.NEGATIVE_INFINITY;

  for (const session of scan.sessions) {
    if (session.app !== app || session.lastActiveAt === null) continue;
    const timestamp = Date.parse(session.lastActiveAt);
    if (Number.isNaN(timestamp) || timestamp <= latestTimestamp) continue;
    latest = session.lastActiveAt;
    latestTimestamp = timestamp;
  }

  return latest;
}

function sessionScanErrorMessage(reason: unknown): string {
  return reason instanceof Error && reason.message ? reason.message : "未提供具体原因";
}

function LocalSessionRow({ app, scan }: { app: AppKind; scan: SessionScan }) {
  const count = scan.sessions.filter((session) => session.app === app).length;
  const latest = latestSessionActivity(scan, app);
  const issues = scan.issues.filter((issue) => issue.app === app);

  return (
    <article className="asb-session-overview-row" aria-label={`${clientName(app)} 本机会话`}>
      <header className="asb-session-overview-client">
        <ClientLogo app={app} className="asb-session-overview-logo" />
        <h3>{clientName(app)}</h3>
      </header>
      <dl className="asb-session-overview-values">
        <div>
          <dt>已发现</dt>
          <dd>{count} 个本机会话</dd>
        </div>
        <div>
          <dt>最近文件更新</dt>
          <dd>{latest ? <Time iso={latest} /> : "没有可读取的记录"}</dd>
        </div>
      </dl>
      {issues.length > 0 && (
        <ul className="asb-session-overview-issues" aria-label={`${clientName(app)} 会话扫描提示`}>
          {issues.map((issue) => (
            <li key={`${issue.app}-${issue.message}`} className="asb-warn-text">
              {issue.message}
            </li>
          ))}
        </ul>
      )}
    </article>
  );
}

/** An explicit, read-only overview of local session file metadata. */
function LocalSessionsOverview({ onOpenSessions }: { onOpenSessions: () => void }) {
  const [scan, setScan] = useState<SessionScan | null>(null);
  const [loading, setLoading] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const requestRef = useRef<Promise<SessionScan> | null>(null);

  const readSessions = async () => {
    if (requestRef.current !== null) return;

    setLoading(true);
    setScanError(null);
    const request = Promise.resolve().then(() => listSessions());
    requestRef.current = request;

    try {
      setScan(await request);
    } catch (reason) {
      setScan(null);
      setScanError(sessionScanErrorMessage(reason));
    } finally {
      if (requestRef.current === request) requestRef.current = null;
      setLoading(false);
    }
  };

  return (
    <section className="asb-panel asb-session-overview" aria-label="本机会话">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">本机会话</h2>
        <div className="asb-panel-actions">
          <button type="button" className="asb-btn-secondary" onClick={onOpenSessions}>
            查看会话
          </button>
          <button
            type="button"
            className="asb-btn-secondary"
            disabled={loading}
            onClick={() => void readSessions()}
          >
            {loading ? "读取中…" : scan === null ? "读取本机会话" : "刷新本机会话"}
          </button>
        </div>
      </div>
      {loading && scan === null && <p className="asb-empty" role="status">正在读取本机会话</p>}
      {scan === null && !loading && scanError === null && <p className="asb-empty">尚未读取本机会话</p>}
      {scanError && <p className="asb-warn-text" role="alert">无法读取本机会话：{scanError}</p>}
      {scan !== null && (
        <>
          <div className="asb-session-overview-list">
            {SESSION_APPS.map((app) => (
              <LocalSessionRow key={app} app={app} scan={scan} />
            ))}
          </div>
          <p className="asb-session-overview-note">时间取本地会话文件的最后修改时间。</p>
        </>
      )}
    </section>
  );
}

/** Overview: the dual-client relay plus per-client configuration status. */
export function OverviewPage({
  statuses,
  locks,
  selectedProfile,
  canSwitch,
  busy,
  relayHidden,
  onPreview,
  onRequestSwitch,
  onRefresh,
  onRecoverLock,
  onOpenSessions,
}: OverviewPageProps) {
  return (
    <>
      {!relayHidden && (
        <DualRelay
          routes={routesFrom(statuses ?? [])}
          selectedProfile={selectedProfile}
          canSwitch={canSwitch}
          busy={busy}
          onPreview={onPreview}
          onSwitch={onRequestSwitch}
        />
      )}
      <section className="asb-panel" aria-label="配置状态">
        <div className="asb-panel-heading">
          <h2 className="asb-panel-title">配置状态</h2>
          <button type="button" className="asb-btn-secondary" disabled={busy} onClick={onRefresh}>
            刷新状态
          </button>
        </div>
        <div className="asb-status-grid">
          {(statuses ?? []).map((status) => (
            <ConfigStatusCard
              key={status.app}
              status={status}
              lock={locks[status.app]}
              busy={busy}
              onRecoverLock={onRecoverLock}
            />
          ))}
        </div>
        {statuses === null && <p className="asb-empty">加载中</p>}
      </section>
      <LocalSessionsOverview onOpenSessions={onOpenSessions} />
    </>
  );
}
