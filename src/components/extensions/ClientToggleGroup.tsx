import type { AppKind, ExtensionListItem } from "../../api/client";
import {
  clientBindingState,
  itemSupportsClient,
} from "../../app/extensions/deployment-state";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Checkbox } from "../Checkbox";
import { Tooltip } from "../Tooltip";
import { clientSummary } from "./labels";
import { MANAGEMENT_CLIENTS } from "./client-presentation";

interface Props {
  item: ExtensionListItem;
  busy: boolean;
  pendingClients?: readonly AppKind[];
  /** The typed write pipeline applies intent and handles sensitive confirmation. */
  onToggle: (client: AppKind) => void;
}

/** Mixed means some existing scopes are disabled. The accessible name also
 * carries the file state, independently of the enable/disable intent. */
export function ClientToggleGroup({ item, busy, pendingClients = [], onToggle }: Props) {
  const { t } = useI18n();
  return (
    <div className="asb-ext-clienttoggles" role="group" aria-label={t("extensions.clientToggles.aria")}>
      {MANAGEMENT_CLIENTS.map((client) => {
        const supported = itemSupportsClient(item, client);
        const state = clientBindingState(item, client);
        const label = supported
          ? t("extensions.clientToggle.supported", { client: clientName(client), state: clientSummary(item, client) })
          : t("extensions.clientToggle.unsupported", { client: clientName(client) });
        return (
          <Tooltip key={client} label={label} side="bottom">
            <Checkbox
              label={clientName(client)}
              ariaLabel={label}
              checked={state.all}
              indeterminate={state.partial}
              ariaBusy={pendingClients.includes(client)}
              disabled={busy || !supported}
              onChange={() => onToggle(client)}
            />
          </Tooltip>
        );
      })}
    </div>
  );
}
