import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getChangelogSection, type UpdateChannel, type UpdateCheck } from "../api/client";
import type { UpdateDownloadView } from "../app/update-download-state";
import { MarkdownContent } from "./Markdown";
import { Button } from "./Button";
import { CollapsibleSection } from "./Collapsible";
import { Time } from "./Time";
import { UpdateDownloadPanel } from "./UpdateDownloadPanel";
import { useI18n } from "../i18n";
import { toast, toastMessage } from "./use-toast";

const releasesUrl = "https://github.com/y4Nkk/agent-switchboard/releases/latest";

interface Props {
  channel: UpdateChannel | null;
  appVersion: string | null;
  result: UpdateCheck | null;
  busy: boolean;
  installing: boolean;
  download: UpdateDownloadView;
  checkedAt: string | null;
  restartRequired: boolean;
  onCheck: () => void;
  onInstall: () => void;
  onCancel: () => void;
  onRestart: () => void;
}

function ReleaseNotes({ notes, version }: { notes: string; version: string }) {
  const { t } = useI18n();
  return <div className="asb-update-notes asb-markdown" aria-label={t("backup.update.notesAria", { version })}>
    <MarkdownContent text={notes} />
  </div>;
}

function useCurrentNotes(appVersion: string | null) {
  const [currentNotes, setCurrentNotes] = useState<{ version: string; notes: string } | null>(null);
  useEffect(() => {
    if (appVersion === null) return;
    let current = true;
    void getChangelogSection(appVersion).then((notes) => {
      if (current) setCurrentNotes(notes ? { version: appVersion, notes } : null);
    }).catch(() => { if (current) setCurrentNotes(null); });
    return () => { current = false; };
  }, [appVersion]);
  return currentNotes?.version === appVersion ? currentNotes : null;
}

function openReleasePage() {
  void openUrl(releasesUrl).catch(() => {
    toast({ kind: "error", title: toastMessage("backup.update.releasePageError"),
      description: toastMessage("backup.update.releasePageErrorDetail") });
  });
}

function UpdateActions(props: Props) {
  const { t } = useI18n();
  const { restartRequired, installing, result, busy, download } = props;
  const retry = download.task?.phase === "cancelled" || download.task?.phase === "failed";
  return <div className="asb-panel-actions">
    {restartRequired ? <Button variant="primary" disabled={installing} onClick={props.onRestart}>
      {t("backup.update.restart")}
    </Button> : result && !installing && !retry ? <Button variant="primary" disabled={busy} onClick={props.onInstall}>
      {t("backup.update.downloadInstall")}
    </Button> : null}
    <Button variant="secondary" disabled={busy || installing || restartRequired} onClick={props.onCheck}>
      {t("backup.update.checkUpdate")}
    </Button>
    <Button variant="secondary" onClick={openReleasePage}>{t("backup.update.releasePage")}</Button>
  </div>;
}

function StoreUpdateSection({ appVersion }: { appVersion: string | null }) {
  const { t } = useI18n();
  return <section className="asb-app-settings-group" aria-labelledby="software-update">
    <h3 id="software-update" className="asb-section-title">{t("backup.update.title")}</h3>
    <div className="asb-app-setting-copy">
      <span className="asb-checkbox-label">{t("backup.update.storeManaged")}</span>
      <span className="asb-app-setting-detail">
        {appVersion ? <>{t("backup.update.currentVersion", { version: appVersion })} · </> : null}
        {t("backup.update.storeAuto")}
      </span>
    </div>
  </section>;
}

/** Task progress precedes release notes, so cancellation stays in view. */
export function UpdateSection(props: Props) {
  const { t } = useI18n();
  const { channel, appVersion, result, busy, installing, checkedAt, restartRequired } = props;
  const currentNotes = useCurrentNotes(appVersion);
  const [changelogOpen, setChangelogOpen] = useState(false);
  if (channel === "microsoftStore") return <StoreUpdateSection appVersion={appVersion} />;
  const label = restartRequired ? t(installing ? "backup.update.restarting" : "backup.update.restartRequired")
    : result ? t("backup.update.newVersion", { version: result.latestVersion })
    : busy ? t("backup.update.checking") : t(checkedAt ? "backup.update.upToDate" : "backup.update.check");
  return <section className="asb-app-settings-group" aria-labelledby="software-update">
    <h3 id="software-update" className="asb-section-title">{t("backup.update.title")}</h3>
    <div className="asb-app-setting-row asb-update-summary">
      <div className="asb-app-setting-copy">
        <span className="asb-checkbox-label">{label}</span>
        {(appVersion || checkedAt) && <span className="asb-app-setting-detail">
          {appVersion ? t("backup.update.currentVersion", { version: appVersion }) : null}
          {appVersion && checkedAt ? " · " : null}
          {checkedAt && <>{t("backup.update.checkedAt")} <Time iso={checkedAt} /></>}
        </span>}
      </div>
      <UpdateActions {...props} />
    </div>
    <UpdateDownloadPanel download={props.download} busy={busy} onCancel={props.onCancel} onRetry={props.onInstall} />
    {result?.releaseNotes && <ReleaseNotes notes={result.releaseNotes} version={result.latestVersion} />}
    {currentNotes && <CollapsibleSection quiet open={changelogOpen} onOpenChange={setChangelogOpen}
      label={t("backup.update.currentNotes")}>
      <ReleaseNotes notes={currentNotes.notes} version={currentNotes.version} />
    </CollapsibleSection>}
  </section>;
}
