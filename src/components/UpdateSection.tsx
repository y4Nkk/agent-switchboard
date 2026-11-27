import { openUrl } from "@tauri-apps/plugin-opener";
import { applicationUpdateView, UPDATE_PHASE_LABELS, type ApplicationUpdateControl } from "../app/update-state";
import { ReleaseNotes } from "./ReleaseNotes";
import { Button } from "./Button";
import { SettingsGroup, SettingsRow } from "./SettingsLayout";
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
    {view.installed ? <Button variant="primary" disabled={!view.canRestart} onClick={update.onRequestRestart}>
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
  const { t, language } = useI18n();
  const { state } = update;
  const view = applicationUpdateView(state);
  const { updateCheck: result, appVersion, lastCheckedAt } = state;
  const store = view.phase === "storeManaged";
  const label = store ? t("backup.update.storeManaged") : view.phase === "available" && result
    ? t("backup.update.newVersion", { version: result.latestVersion }) : t(UPDATE_PHASE_LABELS[view.phase]);
  const advisory = view.phase === "available" && result ? result.advisory : null;
  const reason = advisory && advisory.severity !== "routine"
    ? (language === "zh-CN" ? advisory.reasonZh : advisory.reasonEn) : null;
  return <SettingsGroup title={t("backup.update.title")}>
    <SettingsRow wide={store} label={<>
      {label}
      {advisory && <span className={"asb-update-advisory is-" + advisory.severity}
        data-advisory={advisory.severity}>{t(`backup.update.advisory.${advisory.severity}`)}</span>}
    </>} description={(reason != null || appVersion || lastCheckedAt || store) && <>
      {reason != null && <span className={"asb-update-advisory-reason is-" + advisory?.severity}>{reason}</span>}
      {(appVersion || lastCheckedAt || store) && <span>
        {appVersion ? t("backup.update.currentVersion", { version: appVersion }) : null}
        {appVersion && (lastCheckedAt || store) ? " · " : null}
        {store ? t("backup.update.storeAuto") : lastCheckedAt && <>{t("backup.update.checkedAt")} <Time iso={lastCheckedAt} /></>}
      </span>}
    </>}>
      {!store && <UpdateActions update={update} busy={busy} />}
    </SettingsRow>
    {view.error != null && view.phase !== "failed" && <div className="asb-fail-text" role="alert">
      <CommandErrorLines error={view.error} />
    </div>}
    {!store && <UpdateDownloadPanel download={state} onCancel={update.onCancel} />}
    {!store && result?.releaseNotes && <ReleaseNotes notes={result.releaseNotes} version={result.latestVersion} />}
  </SettingsGroup>;
}
