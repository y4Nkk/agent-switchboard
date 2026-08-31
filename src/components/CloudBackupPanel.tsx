import { useEffect, useState } from "react";
import {
  getCloudBackupSetupSql,
  type CloudBackupSettings,
  type CommandError,
} from "../api/client";
import { ConfirmSheet } from "./ConfirmSheet";
import { toast } from "./use-toast";

type PendingOperation = "upload" | "restore" | null;

interface Props {
  settings: CloudBackupSettings | null;
  loaded: boolean;
  busy: boolean;
  onSave: (settings: CloudBackupSettings) => Promise<boolean>;
  onUpload: (accountPassword: string, backupPassword: string) => Promise<boolean>;
  onRestore: (accountPassword: string, backupPassword: string) => Promise<boolean>;
}

/** User-configured, encrypted profile-store backup. The password inputs stay
 * in component state only and are cleared after a successful remote action. */
export function CloudBackupPanel({
  settings,
  loaded,
  busy,
  onSave,
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

  return (
    <section className="asb-cloud-backup">
      <p className="asb-scope-note">
        供应商档案会先在本机加密，再上传到你自己的 Supabase 项目；Supabase 登录密码和备份密码不会保存。
      </p>
      {!loaded ? (
        <p className="asb-empty">加载云端备份设置</p>
      ) : (
        <>
          <form
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
              <span>Supabase 登录邮箱</span>
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
            <div className="asb-form-actions">
              <button type="button" className="asb-btn-secondary" disabled={busy} onClick={revealSetupSql}>
                {setupSql === null ? "显示初始化 SQL" : "收起初始化 SQL"}
              </button>
              <button type="submit" className="asb-btn-primary" disabled={busy}>
                保存连接
              </button>
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
              <span>Supabase 登录密码</span>
              <input
                className="asb-input"
                type="password"
                autoComplete="current-password"
                value={accountPassword}
                disabled={busy}
                onChange={(event) => setAccountPassword(event.target.value)}
              />
            </label>
            <label className="asb-field">
              <span>备份密码</span>
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
              <button
                type="button"
                className="asb-btn-secondary"
                disabled={busy || settings === null}
                onClick={() => setPending("restore")}
              >
                从云端恢复
              </button>
              <button
                type="button"
                className="asb-btn-primary"
                disabled={busy || settings === null}
                onClick={() => setPending("upload")}
              >
                备份到云端
              </button>
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
            "Supabase 登录密码和备份密码不会保存。",
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
