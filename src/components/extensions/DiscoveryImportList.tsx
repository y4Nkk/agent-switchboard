import type { ObservedExtension } from "../../api/client";
import { discoveryImportMode } from "../../app/extensions/useDiscoveryImport";
import { clientName } from "../../lib/client-name";
import { useI18n, type TFunction } from "../../i18n";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { ClientLogo } from "../ClientLogo";
import { Eye } from "lucide-react";

export function discoveryOrigin(item: ObservedExtension, projects: ReadonlyMap<string, string>, t: TFunction) {
  if (item.origin.origin === "projectRoot") {
    return t("importDiscovery.origin.project", { name: projects.get(item.origin.projectId) ?? t("importDiscovery.origin.registeredProject") });
  }
  if (item.origin.origin === "legacyRoot") return t("importDiscovery.origin.legacyRoot");
  if (item.origin.origin === "managed") return t("importDiscovery.origin.managed");
  return t("importDiscovery.origin.userScope", { client: clientName(item.client) });
}

interface Props {
  rows: ObservedExtension[];
  selected: ReadonlySet<string>;
  busy: boolean;
  projects: ReadonlyMap<string, string>;
  warnings: ReadonlyMap<string, number>;
  onSelect: (id: string, checked: boolean) => void;
  onWarning: (id: string) => void;
  onViewDetails: (item: ObservedExtension) => void;
}

function ImportRow({ item, props }: { item: ObservedExtension; props: Props }) {
  const { t } = useI18n();
  const mode = discoveryImportMode(item);
  const warningCount = props.warnings.get(item.observationId) ?? 0;
  const reason = !mode && !item.managed && !item.actions.import.inLibrary
    ? item.actions.takeover.reason ?? item.actions.import.reason : null;
  return (
    <li className="asb-ext-import-row">
      <div className="asb-ext-import-identity">
        <Checkbox label={item.name} ariaLabel={t("importDiscovery.row.selectAria", { name: item.name, client: clientName(item.client) })}
          checked={props.selected.has(item.observationId)} disabled={props.busy || mode === null}
          onChange={(checked) => props.onSelect(item.observationId, checked)} />
        {item.description && <p className="asb-ext-import-description">{item.description}</p>}
        <p className="asb-ext-import-origin">
          <ClientLogo app={item.client} className="asb-ext-import-client-logo" />
          {discoveryOrigin(item, props.projects, t)}
          {item.transport && <span>{item.transport}</span>}
        </p>
        {reason && <p className="asb-warn-text asb-ext-import-description">{reason}</p>}
      </div>
      <div className="asb-ext-import-status">
        {item.managed ? <span>{t("importDiscovery.row.managed")}</span> : mode === "copy" ? <span>{t("importDiscovery.row.copyOnly")}</span>
          : mode === "manage" ? <span>{t("importDiscovery.row.keepInstall")}</span> : item.actions.import.inLibrary ? <span>{t("importDiscovery.row.inLibrary")}</span> : null}
        {warningCount > 0 && <Button variant="ghost" size="compact" className="asb-warn-text"
          aria-label={t("importDiscovery.row.warningsAria", { name: item.name, count: warningCount })}
          onClick={() => props.onWarning(item.observationId)}>{t("importDiscovery.row.warnings", { count: warningCount })}</Button>}
        {item.actions.managedDefinitionId && <Button variant="icon" size="compact"
          aria-label={t("importDiscovery.row.detailsAria", { name: item.name })} onClick={() => props.onViewDetails(item)}><Eye /></Button>}
      </div>
    </li>
  );
}

export function DiscoveryImportList(props: Props) {
  const { t } = useI18n();
  return <ul className="asb-ext-import-list" aria-label={t("importDiscovery.row.listAria")}>
    {props.rows.map((item) => <ImportRow key={item.observationId} item={item} props={props} />)}
  </ul>;
}
