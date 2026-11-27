import { CommandErrorLines } from "../app/notifications";
import { openUrl } from "@tauri-apps/plugin-opener";
import { type MouseEvent, useEffect, useRef, useState } from "react";
import {
  getCloudBackupSetupSql,
  type CloudBackupSettings,
  type CommandError,
} from "../api/client";
import { Button } from "./Button";
import { BackupArchiveArt } from "./BackupArchiveArt";
import { SettingsGroup, SettingsRow } from "./SettingsLayout";
import { AppDialog } from "./AppDialog";
import { CodePreview } from "./CodePreview";
import { ConfirmSheet } from "./ConfirmSheet";
import { Input } from "./Input";
import { ExportIcon, UpdateIcon } from "./icons";
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

/** The first-run Supabase walkthrough: a read-and-leave task in its own
    dialog, closed before the connection form on the page is filled in. */
function CloudBackupGuide({ projectUrl, onClose }: { projectUrl: string; onClose: () => void }) {
  const { t } = useI18n();
  const dashboardLinks = supabaseDashboardLinks(projectUrl);
  const openGuideLink = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    void openUrl(event.currentTarget.href).catch((error) => toast({ kind: "error",
      title: toastMessage("backup.cloud.guideOpenFailed"), description: errorText(error, t) }));
  };

  return (
    <AppDialog title={t("backup.cloud.guideTitle")} busy={false} onClose={onClose}>
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
          <SetupSql />
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
    </AppDialog>
  );
}

/** The SQL the third guide step runs once, read when the dialog mounts. */
function SetupSql() {
  const { t } = useI18n();
  const [sql, setSql] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    getCloudBackupSetupSql()
      .then((next) => {
        if (active) setSql(next);
      })
      .catch((caught) => {
        if (!active) return;
        const error = caught as CommandError;
        toast({ kind: "error", title: toastMessage("backup.cloud.setupSqlError"), description: <CommandErrorLines error={error} /> });
      });
    return () => {
      active = false;
    };
  }, []);
  return sql === null ? null : <CodePreview target={t("backup.cloud.setupSqlTitle")} content={sql} />;
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

  return <SettingsGroup title={t("backup.cloud.connection")}>
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
  </SettingsGroup>;
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
  return <SettingsGroup title={t("backup.cloud.backupOrRestore")}>
    <SettingsRow label={t("backup.cloud.backupPassword")}
      details={!saved && <p className="asb-field-help">{t("backup.cloud.saveBeforeTransfer")}</p>}>
      <Input type="password" aria-label={t("backup.cloud.backupPassword")} autoComplete="new-password" minLength={8} value={password} disabled={busy}
        onChange={(event) => onPasswordChange(event.target.value)} />
    </SettingsRow>
    <div className="asb-cloud-backup-actions">
      <Button variant="primary" disabled={busy || !saved} onClick={() => onReview("upload")}>
        <ExportIcon />{t("backup.cloud.upload")}
      </Button>
      <Button variant="dangerText" disabled={busy || !saved} onClick={() => onReview("restore")}>
        <UpdateIcon />{t("backup.cloud.restore")}
      </Button>
    </div>
  </SettingsGroup>;
}

export function CloudBackupPanel({ settings, loaded, active, visible, busy, onSave, onTestConnection, onUpload, onRestore }: Props) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<CloudBackupSettings>({ projectUrl: "", publishableKey: "", email: "" });
  const [accountPassword, setAccountPassword] = useState("");
  const [backupPassword, setBackupPassword] = useState("");
  const [pending, setPending] = useState<PendingOperation | null>(null);
  const [guideOpen, setGuideOpen] = useState(false);
  const baseline = useRef<CloudBackupSettings | null>(null);

  useEffect(() => {
    if (active || busy) return;
    setAccountPassword("");
    setBackupPassword("");
    setPending(null);
  }, [active, busy]);

  /* The tab stays mounted while hidden; an overlay left open there would keep
     the rest of the app inert once the user returns. */
  useEffect(() => { if (!active) setGuideOpen(false); }, [active]);

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
        <Button variant="secondary" onClick={() => setGuideOpen(true)}>{t("backup.cloud.guideTitle")}</Button>
      </div>
    </div>
    {!loaded ? (
      <div className="asb-cloud-backup-loading" role="status" aria-label={t("backup.cloud.loadingAria")}>
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
        <div className="asb-skeleton" />
      </div>
    ) : (
      <div className="asb-settings-groups">
        <CloudBackupConnection busy={busy} draft={draft} onDraftChange={setDraft}
          password={accountPassword} onPasswordChange={setAccountPassword} onSave={onSave} onTestConnection={onTestConnection} />
        <CloudBackupTransfer busy={busy} saved={settings !== null} password={backupPassword}
          onPasswordChange={setBackupPassword} onReview={setPending} />
      </div>
    )}
    {active && guideOpen && <CloudBackupGuide projectUrl={draft.projectUrl} onClose={() => setGuideOpen(false)} />}
    {pending && <CloudBackupReview pending={pending} busy={busy}
      onConfirm={() => void confirm()} onClose={() => setPending(null)} />}
  </section>;
}
