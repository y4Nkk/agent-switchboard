import { openUrl } from "@tauri-apps/plugin-opener";
import { applicationUpdateView, UPDATE_PHASE_LABELS, type ApplicationUpdateControl } from "../app/update-state";
import { ReleaseNotes } from "./ReleaseNotes";
import { Button } from "./Button";
import { Time } from "./Time";
import { UpdateDownloadPanel } from "./UpdateDownloadPanel";
import { useI18n } from "../i18n";
import { toast, toastMessage } from "./use-toast";
import { CommandErrorLines } from "../app/notifications";

const releasesUrl = "https://github.com/y4Nkk/agent-switchboard/releases/latest";

function openReleasePage() {
  void openUrl(releasesUrl).catch(() => {
    toast({ kind: "error", title: toastMessage("backup.update.releasePageError"),
      description: toastMessage("backup.update.releasePageErrorDetail") });
  });
}

function UpdateActions({ update, busy }: { update: ApplicationUpdateControl; busy: boolean }) {
  const { t } = useI18n();
  const { state } = update;
  const view = applicationUpdateView(state);
  return <div className="asb-panel-actions">
    {view.installed ? <Button variant="primary" disabled={!view.canRestart} onClick={update.onRestart}>
      {t("backup.update.restart")}
    </Button> : state.updateCheck && !view.busy ? <Button variant="primary" disabled={busy || !view.canInstall} onClick={update.onInstall}>
      {t(view.download.retry ? "backup.update.retryDownload" : "backup.update.downloadInstall")}
    </Button> : null}
    <Button variant="secondary" disabled={busy || !view.canCheck} onClick={update.onCheck}>
      {t(view.phase === "channelFailed" ? "backup.update.retryChannel" : "backup.update.checkUpdate")}
    </Button>
    <Button variant="secondary" onClick={openReleasePage}>{t("backup.update.releasePage")}</Button>
  </div>;
}

/** The shared state projection owns stages and action availability on both surfaces. */
export function UpdateSection({ update, busy }: { update: ApplicationUpdateControl; busy: boolean }) {
  const { t } = useI18n();
  const { state } = update;
  const view = applicationUpdateView(state);
  const { updateCheck: result, appVersion, lastCheckedAt } = state;
  const store = view.phase === "storeManaged";
  const label = store ? t("backup.update.storeManaged") : view.phase === "available" && result
    ? t("backup.update.newVersion", { version: result.latestVersion }) : t(UPDATE_PHASE_LABELS[view.phase]);
  return <section className="asb-app-settings-group" aria-labelledby="software-update">
    <h3 id="software-update" className="asb-section-title">{t("backup.update.title")}</h3>
    <div className="asb-app-setting-row asb-update-summary">
      <div className="asb-app-setting-copy">
        <span className="asb-checkbox-label">{label}</span>
        {(appVersion || lastCheckedAt || store) && <span className="asb-app-setting-detail">
          {appVersion ? t("backup.update.currentVersion", { version: appVersion }) : null}
          {appVersion && (lastCheckedAt || store) ? " · " : null}
          {store ? t("backup.update.storeAuto") : lastCheckedAt && <>{t("backup.update.checkedAt")} <Time iso={lastCheckedAt} /></>}
        </span>}
      </div>
      {!store && <UpdateActions update={update} busy={busy} />}
    </div>
    {view.error != null && view.phase !== "failed" && <div className="asb-fail-text" role="alert">
      <CommandErrorLines error={view.error} />
    </div>}
    {!store && <UpdateDownloadPanel download={state} onCancel={update.onCancel} />}
    {!store && result?.releaseNotes && <ReleaseNotes notes={result.releaseNotes} version={result.latestVersion} />}
  </section>;
}
