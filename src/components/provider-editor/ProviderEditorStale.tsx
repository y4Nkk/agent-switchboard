import { useI18n } from "../../i18n";
import { Button } from "../Button";

export function ProviderEditorStale({ stale, busy, onReload }: { stale: boolean; busy: boolean; onReload: () => void }) {
  const { t } = useI18n();
  if (!stale) return null;
  return <div className="asb-banner asb-banner-warning" role="alert">
    <p>{t("providerEditor.stale")}</p>
    <Button variant="secondary" disabled={busy} onClick={onReload}>{t("modelRoutes.reload")}</Button>
  </div>;
}
