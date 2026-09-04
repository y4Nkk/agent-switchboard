import { openUrl } from "@tauri-apps/plugin-opener";
import { type MouseEvent, useEffect, useRef, useState } from "react";
import {
  getCloudBackupSetupSql,
  type CloudBackupSettings,
  type CommandError,
} from "../api/client";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { toast } from "./use-toast";

type PendingOperation = "upload" | "restore" | null;

const SUPABASE_DASHBOARD_URL = "https://supabase.com/dashboard";

interface SupabaseDashboardLinks {
  project: string;
  dataApi: string;
  sqlEditor: string;
  authUsers: string;
}

function supabaseDashboardLinks(projectUrl: string): SupabaseDashboardLinks {
  const fallback = {
    project: SUPABASE_DASHBOARD_URL,
    dataApi: SUPABASE_DASHBOARD_URL,
    sqlEditor: SUPABASE_DASHBOARD_URL,
    authUsers: SUPABASE_DASHBOARD_URL,
  };
  try {
    const url = new URL(projectUrl.trim());
    const suffix = ".supabase.co";
    if (url.protocol !== "https:" || !url.hostname.endsWith(suffix)) return fallback;
    const projectRef = url.hostname.slice(0, -suffix.length);
    if (!projectRef || projectRef.includes(".")) return fallback;
    const project = `${SUPABASE_DASHBOARD_URL}/project/${encodeURIComponent(projectRef)}`;
    return {
      project,
      dataApi: `${project}/integrations/data_api/overview`,
      sqlEditor: `${project}/sql/new`,
      authUsers: `${project}/auth/users`,
    };
  } catch {
    return fallback;
  }
}

interface Props {
  settings: CloudBackupSettings | null;
  loaded: boolean;
  busy: boolean;
  onSave: (settings: CloudBackupSettings) => Promise<boolean>;
  onTestConnection: (settings: CloudBackupSettings, accountPassword: string) => Promise<boolean>;
  onUpload: (accountPassword: string, backupPassword: string) => Promise<boolean>;
  onRestore: (accountPassword: string, backupPassword: string) => Promise<boolean>;
}

/** User-configured, encrypted profile-store backup. Passwords remain in
 * component state only and clear after a successful remote operation. */
export function CloudBackupPanel({
  settings,
  loaded,
  busy,
  onSave,
  onTestConnection,
  onUpload,
  onRestore,
}: Props) {
  const [draft, setDraft] = useState<CloudBackupSettings>({
    projectUrl: "",
    publishableKey: "",
    email: "",
  });
  const [accountPassword, setAccountPassword] = useState("");
  const [backupPassword, setBackupPassword] = useState("");
  const [pending, setPending] = useState<PendingOperation>(null);
  const [setupSql, setSetupSql] = useState<string | null>(null);
  const connectionForm = useRef<HTMLFormElement>(null);
  const dashboardLinks = supabaseDashboardLinks(draft.projectUrl);

  useEffect(() => {
    if (settings) setDraft(settings);
  }, [settings]);

  const revealSetupSql = () => {
    if (setupSql !== null) {
      setSetupSql(null);
      return;
    }
    void getCloudBackupSetupSql()
      .then(setSetupSql)
      .catch((caught) => {
        const error = caught as CommandError;
        toast({ kind: "error", title: "无法读取初始化 SQL", description: error.message });
      });
  };

  const confirm = async () => {
    if (pending === "upload") {
      const succeeded = await onUpload(accountPassword, backupPassword);
      if (succeeded) {
        setAccountPassword("");
        setBackupPassword("");
        setPending(null);
      }
      return;
    }
    if (pending === "restore") {
      const succeeded = await onRestore(accountPassword, backupPassword);
      if (succeeded) {
        setAccountPassword("");
        setBackupPassword("");
        setPending(null);
      }
    }
  };

  const testConnection = async () => {
    if (!connectionForm.current?.reportValidity()) return;
    await onTestConnection(draft, accountPassword);
  };

  const copySetupSql = async () => {
    if (setupSql === null) return;
    try {
      await navigator.clipboard.writeText(setupSql);
      toast({ kind: "success", title: "已复制初始化 SQL" });
    } catch {
      toast({ kind: "error", title: "无法复制初始化 SQL" });
    }
  };

  const openGuideLink = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    void openUrl(event.currentTarget.href);
  };

  return (
    <section className="asb-cloud-backup">
      <p className="asb-scope-note">
        完整的供应商档案（包括端点、模型和 API 密钥）、通用配置与切换记录会先在本机加密，再上传到你自己的 Supabase 项目。不会备份或直接改写 Codex / Claude Code 原始配置；Dashboard 登录凭据和自行设置的备份密码都不会保存。
      </p>
      <section className="asb-cloud-backup-guide" aria-labelledby="cloud-backup-guide-title">
        <h3 id="cloud-backup-guide-title" className="asb-cloud-backup-guide-title">
          从零配置 Supabase
        </h3>
        <ol className="asb-cloud-backup-guide-list">
          <li>
            <h4>创建项目</h4>
            <p>
              在 <a className="asb-cloud-backup-guide-link" href={SUPABASE_DASHBOARD_URL} onClick={openGuideLink}>Supabase Dashboard</a> 新建项目，等待项目状态变为 Healthy。创建项目时设置的数据库密码只用于数据库连接，不填入本应用。
            </p>
          </li>
          <li>
            <h4>复制项目连接信息</h4>
            <p>
              在 <a className="asb-cloud-backup-guide-link" href={dashboardLinks.project} onClick={openGuideLink}>项目 Dashboard</a> 点击 <code className="asb-code">Connect</code>，复制 <code className="asb-code">Project URL</code> 和 <code className="asb-code">Publishable key</code>。不要使用 Account 的 <code className="asb-code">Access Token</code>、项目 <code className="asb-code">Secret key</code>、<code className="asb-code">service_role</code> 或数据库密码。
            </p>
          </li>
          <li>
            <h4>启用 Data API 并创建备份表</h4>
            <p>
              在 <a className="asb-cloud-backup-guide-link" href={dashboardLinks.dataApi} onClick={openGuideLink}>Integrations → Data API</a> 保持 <code className="asb-code">Enable Data API</code> 开启；再点击下方「显示初始化 SQL」，将全部 SQL 粘贴到 <a className="asb-cloud-backup-guide-link" href={dashboardLinks.sqlEditor} onClick={openGuideLink}>SQL Editor</a> 新查询中，并执行一次。
            </p>
          </li>
          <li>
            <h4>创建项目 Auth 用户</h4>
            <p>
              打开 <a className="asb-cloud-backup-guide-link" href={dashboardLinks.authUsers} onClick={openGuideLink}>Authentication → Users</a>，点击 <code className="asb-code">Add user → Create new user</code>，填写邮箱和新密码并保持 <code className="asb-code">Auto Confirm User</code> 勾选。可使用自己的 Supabase 登录邮箱；Dashboard 或 GitHub 的原有登录密码不能使用。
            </p>
          </li>
          <li>
            <h4>填写并测试</h4>
            <p>
              在下方填写项目地址、Publishable key、项目 Auth 邮箱和新密码，然后点击「测试连接」。测试只验证登录和备份表读取权限；成功后会在当前窗口保留项目 Auth 密码，上传或恢复成功后才清空。
            </p>
          </li>
          <li>
            <h4>保存并备份</h4>
            <p>
              测试成功后点击「保存连接」。首次备份会加密完整的供应商档案、通用配置和切换记录；设置至少 8 位的备份密码，恢复必须使用同一条密码，应用不会保存它。
            </p>
          </li>
        </ol>
      </section>
      {!loaded ? (
        <p className="asb-empty">加载云端备份设置</p>
      ) : (
        <>
          <form
            ref={connectionForm}
            className="asb-form"
            aria-label="Supabase 云端备份设置"
            onSubmit={(event) => {
              event.preventDefault();
              void onSave(draft);
            }}
          >
            <label className="asb-field">
              <span>Supabase 项目地址</span>
              <input
                className="asb-input"
                type="url"
                required
                placeholder="https://your-project.supabase.co"
                value={draft.projectUrl}
                disabled={busy}
                onChange={(event) =>
                  setDraft((current) => ({ ...current, projectUrl: event.target.value }))
                }
              />
            </label>
            <label className="asb-field">
              <span>Publishable key</span>
              <input
                className="asb-input"
                type="password"
                required
                autoComplete="off"
                value={draft.publishableKey}
                disabled={busy}
                onChange={(event) =>
                  setDraft((current) => ({ ...current, publishableKey: event.target.value }))
                }
              />
            </label>
            <label className="asb-field">
              <span>项目 Auth 登录邮箱</span>
              <input
                className="asb-input"
                type="email"
                required
                autoComplete="username"
                value={draft.email}
                disabled={busy}
                onChange={(event) =>
                  setDraft((current) => ({ ...current, email: event.target.value }))
                }
              />
            </label>
            <label className="asb-field">
              <span>项目 Auth 登录密码</span>
              <input
                className="asb-input"
                type="password"
                autoComplete="current-password"
                value={accountPassword}
                disabled={busy}
                onChange={(event) => setAccountPassword(event.target.value)}
              />
            </label>
            <div className="asb-form-actions">
              <Button variant="secondary" disabled={busy} onClick={revealSetupSql}>
                {setupSql === null ? "显示初始化 SQL" : "收起初始化 SQL"}
              </Button>
              {setupSql !== null && (
                <Button variant="secondary" disabled={busy} onClick={() => void copySetupSql()}>
                  复制初始化 SQL
                </Button>
              )}
              <Button
                variant="secondary"
                disabled={busy || accountPassword.length === 0}
                onClick={() => void testConnection()}
              >
                测试连接
              </Button>
              <Button type="submit" variant="primary" disabled={busy}>
                保存连接
              </Button>
            </div>
          </form>
          {setupSql !== null && (
            <label className="asb-field">
              <span>在 Supabase SQL Editor 执行一次</span>
              <textarea
                className="asb-input asb-textarea"
                readOnly
                aria-label="Supabase 初始化 SQL"
                value={setupSql}
              />
            </label>
          )}
          <fieldset className="asb-fieldset">
            <legend>备份或恢复</legend>
            <label className="asb-field">
              <span>备份密码（自行设置）</span>
              <input
                className="asb-input"
                type="password"
                autoComplete="new-password"
                minLength={8}
                value={backupPassword}
                disabled={busy}
                onChange={(event) => setBackupPassword(event.target.value)}
              />
            </label>
            <div className="asb-form-actions">
              <Button
                variant="secondary"
                disabled={busy || settings === null}
                onClick={() => setPending("restore")}
              >
                从云端恢复
              </Button>
              <Button
                variant="primary"
                disabled={busy || settings === null}
                onClick={() => setPending("upload")}
              >
                备份到云端
              </Button>
            </div>
          </fieldset>
        </>
      )}
      {pending === "upload" && (
        <ConfirmSheet
          title="确认上传加密云端备份"
          details={[
            "将加密当前供应商档案、通用配置和切换记录。",
            "将替换此 Supabase 账户已有的云端备份。",
            "项目 Auth 登录密码和备份密码不会保存。",
          ]}
          confirmLabel="确认备份"
          onConfirm={() => void confirm()}
          onCancel={() => setPending(null)}
        />
      )}
      {pending === "restore" && (
        <ConfirmSheet
          title="确认从云端恢复"
          details={[
            "将以云端加密备份替换本机供应商档案、通用配置和切换记录。",
            "不会修改 Codex 或 Claude Code 当前实际配置，也不会删除本地文件备份。",
            "恢复后需要重新预览，才能把任一档案应用到客户端配置。",
          ]}
          confirmLabel="确认恢复"
          destructive
          onConfirm={() => void confirm()}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
