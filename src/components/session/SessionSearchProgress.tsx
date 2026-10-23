import type { SessionSearchProgress as Progress } from "../../api/client";
import { useI18n } from "../../i18n";

export function SessionSearchProgress({ progress }: { progress: Progress | null }) {
  const { t } = useI18n();
  const total = progress?.total ?? null;
  const completed = progress?.completed ?? 0;
  const percent = total === null ? null : total === 0 ? 100 : Math.floor(completed / total * 100);
  const label = total === null ? t("sessions.progress.discovering")
    : t("sessions.progress.sessions", { completed, total });
  return <div className="asb-session-progress">
    <div className="asb-session-progress-ring" data-indeterminate={total === null} role="progressbar" aria-label={label}
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}
      aria-valuetext={label}>
      <svg viewBox="0 0 48 48" aria-hidden="true">
        <circle className="asb-session-progress-track" cx="24" cy="24" r="20" />
        <circle className="asb-session-progress-value" cx="24" cy="24" r="20" pathLength="100"
          strokeDasharray={percent === null ? "20 80" : `${percent} 100`} />
      </svg>
      <span aria-hidden="true">{percent === null ? "…" : `${percent}%`}</span>
    </div>
    <div className="asb-session-progress-copy">
      <span>{label}</span>
      {progress?.stage === "indexing" && <span className="asb-scope-note">{t("sessions.progress.indexing")}</span>}
      {progress?.messagesTotal != null && <span className="asb-scope-note">
        {t("sessions.progress.messages", { completed: progress.messagesCompleted, total: progress.messagesTotal })}
      </span>}
    </div>
  </div>;
}
