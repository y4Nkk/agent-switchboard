import { useId } from "react";
import type { ClientSoftware } from "../../api/client-software";
import { useI18n, type TFunction } from "../../i18n";
import { Button } from "../Button";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { ToolTrack, type ToolStation } from "../client-management/ToolTrack";
import { SoftwareArt } from "./SoftwareArt";
import { SoftwareDetails } from "./SoftwareDetails";

type SoftwareTone = "warning" | "update" | "current" | "neutral";

/** The short status conclusion and the tone that restates it: a release or
 * policy problem, an update the backend allows, or a channel-current reading.
 * The tone decides nothing; `updateAllowed` and the plan stay with the backend. */
function softwareStatus(item: ClientSoftware, t: TFunction): { label: string; tone: SoftwareTone } {
  let label: string;
  if (item.form !== "cli") label = t("software.summary.hostManaged");
  else if (item.releaseError) label = t("software.summary.checkFailed");
  else if (item.reason && item.reason !== "current" && item.reason !== "ahead") label = t(`software.summary.${item.reason}`);
  else label = t(`software.summary.${item.comparison}`);
  const tone: SoftwareTone = item.releaseError || item.policy.issue ? "warning"
    : item.updateAllowed ? "update"
    : item.comparison === "current" ? "current" : "neutral";
  return { label, tone };
}

/** The version readings on one rail. The link after the local reading restates
 * the backend's comparison; the channel and the official release are only
 * compared as text, so two different strings read as "awaiting". */
function versionStations(item: ClientSoftware, t: TFunction): ToolStation[] {
  const unknown = t("software.unknown");
  const local: ToolStation = { label: t("software.local"), value: item.version ?? unknown,
    state: item.version ? "known" : "unknown" };
  if (item.form !== "cli") return [local];
  const channel = item.available?.version ?? null;
  const official = item.official?.version ?? null;
  return [
    { ...local, link: item.comparison === "updateAvailable" ? "upgrade" : item.comparison === "unknown" ? "await" : "same" },
    { label: t("software.channelVersion"), value: channel ?? unknown, state: channel ? "known" : "unknown",
      link: channel !== null && channel === official ? "same" : "await" },
    { label: t("software.official"), value: official ?? unknown, state: official ? "known" : "unknown" },
  ];
}

interface Props {
  item: ClientSoftware;
  disabled: boolean;
  /** ASB launches this installation, so it takes the stage. */
  featured?: boolean;
  /** One card's details are open at a time, like the table's expansion rows. */
  expanded: boolean;
  onExpandedChange: (open: boolean) => void;
  onUpdate: (id: string) => void;
}

/** One installation as a card: its device, the version rail, the status
 * conclusion and the actions. The featured variant is the same markup laid
 * out by the stylesheet. */
export function SoftwareCard({ item, disabled, featured = false, expanded, onExpandedChange, onUpdate }: Props) {
  const { t } = useI18n();
  const detailsId = useId();
  const status = softwareStatus(item, t);
  const channel = item.form === "cli" && item.channel ? ` · ${item.channel}` : "";
  return <article className="asb-tool-card asb-software-card" data-featured={featured || undefined} data-tone={status.tone}>
    <div className="asb-software-ident">
      <div className="asb-software-art"><SoftwareArt form={item.form} /></div>
      <div className="asb-software-titles">
        {featured && <span className="asb-status-pill is-ok">
          <span className="asb-status-pill-dot" aria-hidden="true" />{t("software.active")}
        </span>}
        <h4 className="asb-software-name">{t(`software.form.${item.form}`)}</h4>
        <span className="asb-software-meta">{t(`software.manager.${item.manager}`)}{channel}</span>
      </div>
    </div>
    <ToolTrack reading emphasis={featured} stations={versionStations(item, t)} />
    <div className="asb-software-foot">
      <p className="asb-tool-status"><span className="asb-tool-lamp" aria-hidden="true" />{status.label}</p>
      <div className="asb-software-actions">
        {item.updateAllowed && <Button variant="primary" size="compact" disabled={disabled}
          onClick={() => onUpdate(item.id)}>{t("software.update")}</Button>}
        <CollapsibleTrigger open={expanded} contentId={detailsId} label={t("software.detailsAction")}
          onOpenChange={onExpandedChange} />
      </div>
    </div>
    <Collapsible open={expanded} id={detailsId}>
      <SoftwareDetails item={item} />
    </Collapsible>
  </article>;
}
