import type { ClientUpdatePlan } from "../../api/client-software";
import { useI18n } from "../../i18n";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { FactList } from "../FactList";

export function UpdateReview({ plan, busy, error, onClose, onStart }: {
  plan: ClientUpdatePlan; busy: boolean; error: string | null; onClose: () => void; onStart: () => void;
}) {
  const { t } = useI18n();
  return <AppDialog title={t("software.preview")} busy={busy} onClose={onClose} footer={<>
    <Button variant="secondary" disabled={busy} onClick={onClose}>{t("software.dismiss")}</Button>
    <Button variant="primary" disabled={busy} onClick={onStart}>{t("software.confirm")}</Button>
  </>}>
    {error && <p role="alert" className="asb-field-error">{error}</p>}
    <FactList rows={[
      { label: t("software.from"), value: plan.fromVersion },
      { label: t("software.target"), value: plan.targetVersion },
      { label: t("software.channel"), value: `${t(`software.manager.${plan.manager}`)} · ${plan.channel}` },
      { label: t("software.launcher"), value: <code>{plan.launcher}</code> },
      { label: t("software.network"), value: t(`software.proxy.${plan.proxyMode}`) },
    ]} />
    <p className="asb-scope-note">{t(plan.exactTarget ? "software.exact" : "software.channelTarget")}</p>
    <p className="asb-scope-note">{t("software.installNotice")}</p>
    <p className="asb-scope-note">{t("software.installerScope")}</p>
  </AppDialog>;
}
