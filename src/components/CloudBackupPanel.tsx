import { CommandErrorLines } from "../app/notifications";
import { openUrl } from "@tauri-apps/plugin-opener";
import { type MouseEvent, useEffect, useId, useRef, useState } from "react";
import {
  getCloudBackupSetupSql,
  type CloudBackupSettings,
  type CommandError,
} from "../api/client";
import { Button } from "./Button";
import { BackupArchiveArt } from "./BackupArchiveArt";
import { CollapsibleSection } from "./Collapsible";
import { ConfirmSheet } from "./ConfirmSheet";
import { Input } from "./Input";
import { ExportIcon, UpdateIcon } from "./icons";
import { Textarea } from "./Textarea";
import { useI18n } from "../i18n";
import { errorText } from "../i18n/errors";
import { toast, toastMessage } from "./use-toast";

type PendingOperation = "upload" | "restore";

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
  active: boolean;
  visible: boolean;
  settings: CloudBackupSettings | null;
  loaded: boolean;
  busy: boolean;
  onSave: (settings: CloudBackupSettings) => Promise<boolean>;
  onTestConnection: (settings: CloudBackupSettings, accountPassword: string) => Promise<boolean>;
  onUpload: (accountPassword: string, backupPassword: string) => Promise<boolean>;
  onRestore: (accountPassword: string, backupPassword: string) => Promise<boolean>;
}

function CloudBackupGuide({ projectUrl }: { projectUrl: string }) {
  const { t } = useI18n();
  const [guideOpen, setGuideOpen] = useState(false);
  const dashboardLinks = supabaseDashboardLinks(projectUrl);
  const openGuideLink = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    void openUrl(event.currentTarget.href).catch((error) => toast({ kind: "error",
      title: toastMessage("backup.cloud.guideOpenFailed"), description: errorText(error, t) }));
  };

  return (
    <CollapsibleSection open={guideOpen} onOpenChange={setGuideOpen} label={t("backup.cloud.guideTitle")}>
      <ol className="asb-cloud-backup-guide-list">
        <li>
          <h4 className="asb-group-title">{t("backup.cloud.guide.create.title")}</h4>
          <p>
            {t("backup.cloud.guide.create.before")}
            <a className="asb-cloud-backup-guide-link" href={SUPABASE_DASHBOARD_URL} onClick={openGuideLink}>Supabase Dashboard</a>
            {t("backup.cloud.guide.create.after")}
          </p>
        </li>
        <li>
          <h4 className="asb-group-title">{t("backup.cloud.guide.connect.title")}</h4>
          <p>
            {t("backup.cloud.guide.connect.before")}
            <a className="asb-cloud-backup-guide-link" href={dashboardLinks.project} onClick={openGuideLink}>{t("backup.cloud.guide.connect.projectLink")}</a>
            {t("backup.cloud.guide.connect.mid1")}
            <code className="asb-code">Connect</code>
            {t("backup.cloud.guide.connect.mid2")}
            <code className="asb-code">Project URL</code>
            {t("backup.cloud.guide.connect.mid3")}
            <code className="asb-code">Publishable key</code>
            {t("backup.cloud.guide.connect.mid4")}
            <code className="asb-code">Access Token</code>
            {t("backup.cloud.guide.connect.mid5")}
            <code className="asb-code">Secret key</code>
            {t("backup.cloud.guide.connect.mid6")}
            <code className="asb-code">service_role</code>
            {t("backup.cloud.guide.connect.after")}
          </p>
        </li>
        <li>
          <h4 className="asb-group-title">{t("backup.cloud.guide.dataApi.title")}</h4>
          <p>
            {t("backup.cloud.guide.dataApi.before")}
            <a className="asb-cloud-backup-guide-link" href={dashboardLinks.dataApi} onClick={openGuideLink}>Integrations → Data API</a>
            {t("backup.cloud.guide.dataApi.mid1")}
            <code className="asb-code">Enable Data API</code>
            {t("backup.cloud.guide.dataApi.mid2")}
            <a className="asb-cloud-backup-guide-link" href={dashboardLinks.sqlEditor} onClick={openGuideLink}>SQL Editor</a>
            {t("backup.cloud.guide.dataApi.after")}
          </p>
        </li>
        <li>
          <h4 className="asb-group-title">{t("backup.cloud.guide.auth.title")}</h4>
          <p>
            {t("backup.cloud.guide.auth.before")}
            <a className="asb-cloud-backup-guide-link" href={dashboardLinks.authUsers} onClick={openGuideLink}>Authentication → Users</a>
            {t("backup.cloud.guide.auth.mid1")}
            <code className="asb-code">Add user → Create new user</code>
            {t("backup.cloud.guide.auth.mid2")}
            <code className="asb-code">Auto Confirm User</code>
            {t("backup.cloud.guide.auth.after")}
          </p>
        </li>
        <li>
          <h4 className="asb-group-title">{t("backup.cloud.guide.fill.title")}</h4>
          <p>{t("backup.cloud.guide.fill.body")}</p>
        </li>
        <li>
          <h4 className="asb-group-title">{t("backup.cloud.guide.save.title")}</h4>
          <p>{t("backup.cloud.guide.save.body")}</p>
        </li>
      </ol>
    </CollapsibleSection>
  );
}

function SetupSql({ busy }: { busy: boolean }) {
  const { t } = useI18n();
  const sqlId = useId();
  const [setupSql, setSetupSql] = useState<string | null>(null);
  const revealSetupSql = () => {
    if (setupSql !== null) {
      setSetupSql(null);
      return;
    }
    void getCloudBackupSetupSql()
      .then(setSetupSql)
      .catch((caught) => {
        const error = caught as CommandError;
        toast({ kind: "error", title: toastMessage("backup.cloud.setupSqlError"), description: <CommandErrorLines error={error} /> });
      });
  };

  const copySetupSql = async () => {
    if (setupSql === null) return;
    try {
      await navigator.clipboard.writeText(setupSql);
      toast({ kind: "success", title: toastMessage("backup.cloud.setupSqlCopied") });
    } catch {
      toast({ kind: "error", title: toastMessage("backup.cloud.setupSqlCopyFailed") });
    }
  };

  return <div className="asb-cloud-backup-sql">
    <div className="asb-cloud-backup-actions">
      <Button variant="secondary" size="compact" disabled={busy} onClick={revealSetupSql}
        aria-expanded={setupSql !== null} aria-controls={setupSql === null ? undefined : sqlId}>
        {t(setupSql === null ? "backup.cloud.showSetupSql" : "backup.cloud.hideSetupSql")}
      </Button>
      {setupSql !== null && <Button variant="secondary" size="compact" disabled={busy}
        onClick={() => void copySetupSql()}>{t("backup.cloud.copySetupSql")}</Button>}
    </div>
    {setupSql !== null && <label className="asb-field" id={sqlId}>
      <span>{t("backup.cloud.setupSqlLabel")}</span>
      <Textarea readOnly aria-label={t("backup.cloud.setupSqlAria")} value={setupSql} />
    </label>}
  </div>;
}

interface ConnectionProps extends Pick<Props, "busy" | "onSave" | "onTestConnection"> {
  draft: CloudBackupSettings;
  onDraftChange: (draft: CloudBackupSettings) => void;
  password: string;
  onPasswordChange: (password: string) => void;
}

function CloudBackupConnection({ busy, onSave, onTestConnection, draft, onDraftChange, password, onPasswordChange }: ConnectionProps) {
  const { t } = useI18n();
  const form = useRef<HTMLFormElement>(null);

  function testConnection() {
    if (form.current?.reportValidity()) void onTestConnection(draft, password);
  }

  return <div className="asb-cloud-backup-group">
    <h4 className="asb-section-title">{t("backup.cloud.connection")}</h4>
    <form ref={form} className="asb-form" aria-label={t("backup.cloud.formAria")} onSubmit={(event) => {
      event.preventDefault();
      void onSave(draft);
    }}>
      <label className="asb-field">
        <span>{t("backup.cloud.projectUrl")}</span>
        <Input type="url" required placeholder="https://your-project.supabase.co" value={draft.projectUrl}
          disabled={busy} onChange={(event) => onDraftChange({ ...draft, projectUrl: event.target.value })} />
      </label>
      <label className="asb-field">
        <span>Publishable key</span>
        <Input type="password" required autoComplete="off" value={draft.publishableKey} disabled={busy}
          onChange={(event) => onDraftChange({ ...draft, publishableKey: event.target.value })} />
      </label>
      <label className="asb-field">
        <span>{t("backup.cloud.authEmail")}</span>
        <Input type="email" required autoComplete="username" value={draft.email} disabled={busy}
          onChange={(event) => onDraftChange({ ...draft, email: event.target.value })} />
      </label>
      <label className="asb-field">
        <span>{t("backup.cloud.authPassword")}</span>
        <Input type="password" autoComplete="current-password" value={password} disabled={busy}
          onChange={(event) => onPasswordChange(event.target.value)} />
      </label>
      <div className="asb-cloud-backup-actions">
        <Button variant="secondary" disabled={busy || password.length === 0} onClick={testConnection}>
          {t("backup.cloud.testConnection")}
        </Button>
        <Button type="submit" variant="primary" disabled={busy}>{t("backup.cloud.saveConnection")}</Button>
      </div>
    </form>
    <SetupSql busy={busy} />
  </div>;
}

function CloudBackupReview({ pending, busy, onConfirm, onClose }: {
  pending: PendingOperation; busy: boolean; onConfirm: () => void; onClose: () => void;
}) {
  const { t } = useI18n();
  const restore = pending === "restore";
  return <ConfirmSheet title={t(restore ? "backup.cloud.restoreConfirmTitle" : "backup.cloud.uploadConfirmTitle")}
    busy={busy} confirmLabel={t(restore ? "backup.confirmRestore" : "backup.cloud.uploadConfirmConfirm")}
    destructive={restore} onConfirm={onConfirm} onCancel={onClose}>
    <ul className="asb-dialog-details">
      {restore ? <>
        <li>{t("backup.cloud.restoreConfirm1")}</li>
        <li>{t("backup.cloud.restoreConfirm2")}</li>
        <li>{t("backup.cloud.restoreConfirm3")}</li>
      </> : <>
        <li>{t("backup.cloud.uploadConfirm2")}</li>
        <li>{t("backup.cloud.uploadConfirm3")}</li>
      </>}
    </ul>
  </ConfirmSheet>;
}

function CloudBackupTransfer({ busy, saved, password, onPasswordChange, onReview }: {
  busy: boolean; saved: boolean; password: string;
  onPasswordChange: (password: string) => void;
  onReview: (pending: PendingOperation) => void;
}) {
  const { t } = useI18n();
  const headingId = useId();
  return <section className="asb-cloud-backup-group" aria-labelledby={headingId}>
    <h4 className="asb-section-title" id={headingId}>{t("backup.cloud.backupOrRestore")}</h4>
    <label className="asb-field">
      <span>{t("backup.cloud.backupPassword")}</span>
      <Input type="password" autoComplete="new-password" minLength={8} value={password} disabled={busy}
        onChange={(event) => onPasswordChange(event.target.value)} />
    </label>
    {!saved && <p className="asb-field-help">{t("backup.cloud.saveBeforeTransfer")}</p>}
    <div className="asb-cloud-backup-actions">
      <Button variant="primary" disabled={busy || !saved} onClick={() => onReview("upload")}>
        <ExportIcon />{t("backup.cloud.upload")}
      </Button>
      <Button variant="dangerText" disabled={busy || !saved} onClick={() => onReview("restore")}>
        <UpdateIcon />{t("backup.cloud.restore")}
      </Button>
    </div>
  </section>;
}

export function CloudBackupPanel({ settings, loaded, active, visible, busy, onSave, onTestConnection, onUpload, onRestore }: Props) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<CloudBackupSettings>({ projectUrl: "", publishableKey: "", email: "" });
  const [accountPassword, setAccountPassword] = useState("");
  const [backupPassword, setBackupPassword] = useState("");
  const [pending, setPending] = useState<PendingOperation | null>(null);
  const baseline = useRef<CloudBackupSettings | null>(null);

  useEffect(() => {
    if (active || busy) return;
    setAccountPassword("");
    setBackupPassword("");
    setPending(null);
  }, [active, busy]);

  useEffect(() => {
    if (!settings) return;
    const previous = baseline.current;
    baseline.current = settings;
    setDraft((current) => !previous || JSON.stringify(current) === JSON.stringify(previous) ? settings : current);
  }, [settings]);

  async function confirm() {
    if (!pending) return;
    const action = pending === "upload" ? onUpload : onRestore;
    if (await action(accountPassword, backupPassword)) {
      setAccountPassword("");
      setBackupPassword("");
      setPending(null);
    }
  }

  return <section className="asb-cloud-backup">
    <div className="asb-backup-hero">
      <div className="asb-backup-hero-art"><BackupArchiveArt kind="cloud" active={visible} /></div>
      <div className="asb-backup-hero-copy">
        <h4 className="asb-section-title">{t("backup.cloud.encryptedTitle")}</h4>
        <p className="asb-field-help">{t("backup.cloud.scope")}</p>
      </div>
    </div>
    <CloudBackupGuide projectUrl={draft.projectUrl} />
    {!loaded ? (
      <div className="asb-cloud-backup-loading" role="status" aria-label={t("backup.cloud.loadingAria")}>
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
      </div>
    ) : (
      <div className="asb-cloud-backup-controls">
        <CloudBackupConnection busy={busy} draft={draft} onDraftChange={setDraft}
          password={accountPassword} onPasswordChange={setAccountPassword} onSave={onSave} onTestConnection={onTestConnection} />
        <CloudBackupTransfer busy={busy} saved={settings !== null} password={backupPassword}
          onPasswordChange={setBackupPassword} onReview={setPending} />
      </div>
    )}
    {pending && <CloudBackupReview pending={pending} busy={busy}
      onConfirm={() => void confirm()} onClose={() => setPending(null)} />}
  </section>;
}
