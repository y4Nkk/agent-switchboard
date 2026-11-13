import { useI18n, type TFunction } from "../i18n";
import { errorDiagnosticText, errorText } from "../i18n/errors";
import { formatUpdateBytes, UPDATE_DOWNLOAD_LABELS } from "../app/update-download-state";
import { applicationUpdateView, UPDATE_PHASE_LABELS, type ApplicationUpdateControl, type ApplicationUpdateSnapshot } from "../app/update-state";
import { CheckIcon, CloseIcon, ReloadIcon, SettingsIcon, UpdateFailureIcon, UpdateIcon, UpdateSpinnerIcon } from "./icons";
import { Button } from "./Button";
import { Tooltip } from "./Tooltip";

type View = ReturnType<typeof applicationUpdateView>;

function sidebarText(state: ApplicationUpdateSnapshot, view: View, t: TFunction, language: string) {
  const { download, phase } = view;
  const title = t(download.stale ? "update.statusRetry" : UPDATE_PHASE_LABELS[phase]);
  const version = view.version ? t("update.version", { version: view.version }) : null;
  const bytes = formatUpdateBytes(state.task?.downloadedBytes ?? 0, language);
  const reading = state.task?.totalBytes && state.task.totalBytes > 0
    ? `${bytes} / ${formatUpdateBytes(state.task.totalBytes, language)}`
    : t("backup.update.downloadedUnknown", { size: bytes });
  let detail = version;
  if (download.stale && state.task) {
    detail = download.percent !== null ? `${t("update.lastProgress", { percent: download.percent })} · ${bytes}`
      : t("update.lastPhase", { phase: t(UPDATE_DOWNLOAD_LABELS[state.task.phase]) });
  } else if (download.cancelError != null) detail = t("update.cancelFailed");
  else if (phase === "downloading") detail = reading;
  else if (phase === "idle" || phase === "checking" || phase === "initializing") detail = null;
  else if (phase === "current") detail = t("update.checkAgain");
  else if (view.error != null) detail = t("update.viewError");
  else if (phase === "cancelled") detail = t("update.retryDownload");
  else if (phase === "storeManaged") detail = t("backup.update.storeAuto");
  const percent = phase === "downloading" && !download.stale && download.percent !== null ? `${download.percent}%` : null;
  const diagnostics = [view.error, download.readError, download.cancelError].filter((error) => error != null)
    .flatMap((error) => [errorText(error, t), errorDiagnosticText(error, t)]);
  const label = [...new Set([title, version, percent, detail,
    state.task ? reading : null, ...diagnostics, view.showDetails ? t("update.openDetails") : null].filter(Boolean))].join(" · ");
  return { title, detail, percent, label };
}

function UpdateGlyph({ view, taskId }: { view: View; taskId: string | undefined }) {
  const { phase, busy, download } = view;
  const checking = phase === "checking" || phase === "initializing";
  const ring = (busy && !checking) || phase === "installed";
  const mark = phase === "verifying" || phase === "installed" || phase === "current" ? <CheckIcon />
    : phase === "installing" ? <SettingsIcon /> : phase === "restarting" ? <ReloadIcon />
      : phase === "cancelled" || phase === "cancelling" ? <CloseIcon />
        : phase === "failed" || phase === "checkFailed" || phase === "channelFailed" || download.stale ? <UpdateFailureIcon /> : <UpdateIcon />;
  const progress = phase === "installed" ? 100 : download.percent;
  return <span className="asb-updatebtn-glyph" aria-hidden="true">
    {ring ? <>
      <span key={taskId ?? phase} className="asb-updatebtn-rotor" data-spinning={progress === null && !download.stale}>
        <svg className="asb-updatebtn-ring" viewBox="0 0 24 24" data-indeterminate={progress === null}>
          <circle className="asb-updatebtn-track" cx="12" cy="12" r="10" />
          <circle className="asb-updatebtn-value" cx="12" cy="12" r="10" pathLength="100"
            strokeDasharray={progress === null ? "22 78" : "100"} strokeDashoffset={progress === null ? 0 : 100 - progress} />
        </svg>
      </span>
      <span className="asb-updatebtn-mark">{mark}</span>
    </> : <span className="asb-updatebtn-rotor" data-spinning={checking}>
      {checking ? <UpdateSpinnerIcon /> : mark}
    </span>}
    {phase === "available" && <span className="asb-updatebtn-dot" />}
  </span>;
}

export function UpdateButton({ update, collapsed, onOpen }: {
  update: ApplicationUpdateControl; collapsed: boolean; onOpen: () => void;
}) {
  const { t, language } = useI18n();
  const { state, onCheck } = update;
  const view = applicationUpdateView(state);
  const text = sidebarText(state, view, t, language);
  const checking = view.phase === "checking" || view.phase === "initializing";
  return <>
    <Tooltip label={text.label} side="right" enabled={collapsed}>
      <Button variant="unstyled" className="asb-updatebtn" data-phase={view.phase} data-tone={view.tone}
        aria-label={text.label} aria-busy={view.busy || undefined} disabled={checking}
        onClick={view.showDetails ? onOpen : onCheck}>
        <UpdateGlyph view={view} taskId={state.task?.id} />
        <span className="asb-updatebtn-label" aria-hidden="true">
          <span className="asb-updatebtn-summary">
            <span className="asb-updatebtn-title">{text.title}</span>
            {text.percent && <span className="asb-updatebtn-percent asb-num">{text.percent}</span>}
          </span>
          {text.detail && <span className="asb-updatebtn-detail asb-num">{text.detail}</span>}
        </span>
      </Button>
    </Tooltip>
    <span className="asb-updatebtn-status" role={view.busy ? "progressbar" : "status"} aria-label={text.title}
      aria-valuemin={view.busy ? 0 : undefined} aria-valuemax={view.busy ? 100 : undefined}
      aria-valuenow={view.phase === "downloading" && !view.download.stale ? view.download.percent ?? undefined : undefined}
      aria-valuetext={view.busy ? text.label : undefined}>{text.title}</span>
  </>;
}
