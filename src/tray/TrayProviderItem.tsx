import { Check, RefreshCw } from "lucide-react";
import { useMemo } from "react";
import type { TraySnapshot } from "../api/client";
import { useI18n } from "../i18n";
import { TrayUsage } from "./TrayUsage";
import { trayUsageContent } from "./tray-usage";
import { cx } from "../utils/cx";

type TrayProvider = TraySnapshot["providers"][number];

interface Props {
  provider: TrayProvider;
  busy: boolean;
  switchingId: string | null;
  armedId: string | null;
  refreshingId: string | null;
  onSwitch: () => void;
  onRefreshUsage: () => void;
}

/** The row is a container owning the silhouette; the switch action spans
 * every column as one native button and the usage warning carries a second
 * native refresh button. Both stay real buttons because a button cannot be
 * nested; tray.css owns every visual property. */
export function TrayProviderItem({ provider, busy, switchingId, armedId, refreshingId, onSwitch, onRefreshUsage }: Props) {
  const { t, language } = useI18n();
  // Plain formatters consume the committed language, so locale is an input too.
  const content = useMemo(() => trayUsageContent(provider.usage), [provider.usage, language]);
  const armed = armedId === provider.id;
  const confirmLabel = t("tray.provider.confirmSwitch", { name: provider.name });
  const action = provider.active
    ? t("tray.provider.active", { name: provider.name })
    : armed ? confirmLabel : t("tray.provider.switchTo", { name: provider.name });
  const status = provider.active
    ? t("tray.provider.current")
    : switchingId === provider.id ? t("tray.provider.switching")
    : armed ? confirmLabel : t("tray.provider.switch");

  return (
    <div className={cx("tray-provider", provider.active && "is-active", switchingId === provider.id && "is-switching", armed && "is-armed")}>
      <button
        type="button"
        className="tray-provider-main"
        disabled={busy || provider.active}
        aria-label={content.summary ? `${action}，${content.summary}` : action}
        title={[provider.name, content.summary].filter(Boolean).join("\n")}
        onClick={onSwitch}
      >
        <span className="tray-provider-check">{provider.active && <Check size={18} aria-hidden="true" />}</span>
        <span className="tray-provider-name" title={provider.name}>{provider.name}</span>
        <TrayUsage content={content} />
        <span className={cx("tray-provider-switch", provider.active && "is-current")}>
          {status}
        </span>
      </button>
      {content.alert && (
        <button
          type="button"
          className="tray-provider-refresh"
          disabled={busy}
          aria-label={t("tray.usage.refresh")}
          title={t("tray.usage.refresh")}
          onClick={onRefreshUsage}
        >
          <RefreshCw size={14} className={cx(refreshingId === provider.id && "is-busy")} aria-hidden="true" />
        </button>
      )}
    </div>
  );
}
