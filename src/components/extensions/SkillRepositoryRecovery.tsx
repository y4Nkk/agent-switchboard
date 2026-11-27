import { LoaderCircle, Wrench } from "lucide-react";
import type { SkillRepositoryFailedRow, SkillRepositoryRecovery } from "../../api/extensions/skill-sources";
import { useI18n, type TFunction } from "../../i18n";
import { Button } from "../Button";
import { sourceErrorMessage } from "./skill-source-model";
import type { SkillSourceState } from "./useSkillSource";

function recoveryDescription(recovery: SkillRepositoryRecovery, t: TFunction) {
  switch (recovery.kind) {
    case "updateAddress": return t("extensions.repoScan.recoverAddress", { repo: recovery.repo });
    case "narrowScope": return t("extensions.repoScan.recoverScope", { subpath: recovery.subpath });
    case "retry": return t("extensions.repoScan.recoverRetry");
  }
}

export function SkillRepositoryFailure({ row, previous, className }: {
  row: SkillRepositoryFailedRow;
  previous?: boolean;
  className: string;
}) {
  const { t } = useI18n();
  return <p className={className}>
    {sourceErrorMessage(row.message, t("extensions.sources.refreshFailed"))}
    {previous && <> · {t("extensions.repoScan.previousResults")}</>}
    {row.recovery && <span className="asb-repo-recovery-hint">{recoveryDescription(row.recovery, t)}</span>}
  </p>;
}

export function SkillRepositoryRecoveryButton({ state, disabled, compact = false }: {
  state: SkillSourceState;
  disabled: boolean;
  compact?: boolean;
}) {
  const { t } = useI18n();
  if (!state.repairing && state.catalog.recoveries.length === 0) return null;
  return <Button variant="secondary" size={compact ? "compact" : "standard"}
    disabled={disabled || state.repairing || state.catalog.retryAt !== null}
    aria-busy={state.repairing} onClick={() => void state.repair()}>
    {state.repairing ? <LoaderCircle size={16} className="asb-skill-source-spinner" /> : <Wrench size={16} />}
    {t(state.repairing ? "extensions.repoScan.recovering" : "extensions.repoScan.recover")}
  </Button>;
}

export function SkillRepositoryRetryNotice({ retryAt }: { retryAt: number | null }) {
  const { t, language } = useI18n();
  if (retryAt === null) return null;
  const time = new Date(retryAt * 1000).toLocaleString(language, { dateStyle: "short", timeStyle: "medium" });
  return <p className="asb-skill-repository-error" role="status">{t("extensions.repoScan.rateLimited", { time })}</p>;
}
