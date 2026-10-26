import type { Ref } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { ConnectivityIcon, EditIcon, GatewayIcon, TrashIcon, UsageIcon } from "./icons";
import { Tooltip } from "./Tooltip";

export function ProviderEndpoint({ url, link = false }: { url: string; link?: boolean }) {
  let host = url;
  try { host = new URL(url).host; } catch { /* Keep the supplied endpoint readable. */ }
  if (!link) return <span title={url}>{host}</span>;
  return <a className="asb-provider-link" href={url} title={url}
    onClick={(event) => { event.preventDefault(); void openUrl(url); }}>{host}</a>;
}

export function ProviderActivateButton({ name, onActivate }: { name: string; onActivate: () => void }) {
  const { t } = useI18n();
  const label = t("providers.row.activate", { name });
  return <Tooltip label={label}>
    <Button variant="primary" className="asb-row-activate" aria-label={label} onClick={onActivate}>{t("providers.row.activateShort")}</Button>
  </Tooltip>;
}

export function ProviderLoginButton({ name, open, controlsId, onToggle }: {
  name: string; open: boolean; /** Target of aria-controls while the login panel exists. */
  controlsId?: string; onToggle: () => void;
}) {
  const { t } = useI18n();
  const label = open ? t("providers.row.loginCollapse", { name }) : t("providers.row.relogin", { name });
  return <Tooltip label={label}>
    <Button variant="secondary" className={`asb-row-activate${open ? " is-active" : ""}`}
      aria-label={label} aria-expanded={open} aria-controls={open ? controlsId : undefined}
      onClick={onToggle}>{open ? t("providers.row.loginCollapseShort") : t("providers.row.reloginShort")}</Button>
  </Tooltip>;
}

interface Props {
  name: string;
  onRoutes?: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
  test?: { id: string; open: boolean; onToggle: () => void; trigger: Ref<HTMLButtonElement> };
  usage?: { id: string; configured: boolean; open: boolean; onOpen: () => void };
}

/** Both clients use the same actions and disclosure semantics. */
export function ProviderRowActions({ name, onRoutes, onEdit, onDelete, test, usage }: Props) {
  const { t } = useI18n();
  const testLabel = test?.open ? t("providers.row.testCollapse", { name }) : t("providers.row.test", { name });
  const usageLabel = !usage?.configured ? t("providers.row.usageConfigure", { name })
    : usage.open ? t("providers.row.usageCollapse", { name }) : t("providers.row.usageView", { name });
  const editLabel = t("providers.row.edit", { name });
  const deleteLabel = t("providers.row.delete", { name });
  return <>
    {onRoutes && <Tooltip label={t("modelRoutes.open", { name })}>
      <Button variant="icon" aria-label={t("modelRoutes.open", { name })} onClick={onRoutes}><GatewayIcon /></Button>
    </Tooltip>}
    {onEdit && <Tooltip label={editLabel}>
      <Button variant="icon" aria-label={editLabel} onClick={onEdit}><EditIcon /></Button>
    </Tooltip>}
    {test && <Tooltip label={testLabel}>
      <Button ref={test.trigger} variant="icon" className={test.open ? "is-active" : undefined}
        aria-label={testLabel} aria-controls={test.open ? test.id : undefined} aria-expanded={test.open}
        onClick={test.onToggle}><ConnectivityIcon /></Button>
    </Tooltip>}
    {usage && <Tooltip label={usageLabel}>
      <Button variant="icon" className={usage.configured && usage.open ? "is-active" : undefined}
        aria-label={usageLabel} aria-controls={usage.configured && usage.open ? usage.id : undefined}
        aria-expanded={usage.configured ? usage.open : undefined} onClick={usage.onOpen}><UsageIcon /></Button>
    </Tooltip>}
    {onDelete && <Tooltip label={deleteLabel}>
      <Button variant="icon" aria-label={deleteLabel} onClick={onDelete}><TrashIcon /></Button>
    </Tooltip>}
  </>;
}
