import { useEffect, useState, type ReactNode } from "react";
import { EditorFrame } from "../../components/EditorFrame";
import {
  DiscoverPanel,
  DiscoveryImportBar,
  useDiscoveryRows,
} from "../../components/extensions/DiscoverPanel";
import { useDiscoverySelection } from "../../components/extensions/useDiscoverySelection";
import { useI18n } from "../../i18n";
import { ExtensionSearch } from "./ExtensionToolbar";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

interface Props {
  workspace: ExtensionWorkspace;
  /** Persistent failure notice rendered at the top of the scroll body. */
  notice?: ReactNode;
}

/** Scanning existing extensions uses the shared full-page editor frame: search and
 * scan results scroll, while batch selection and the import commit stay in
 * the fixed bottom bar. */
export function ExtensionDiscoveryView({ workspace: w, notice }: Props) {
  const { t } = useI18n();
  const kind = w.nav.kind === "mcp" ? "mcp" : "skill";
  const [search, setSearch] = useState("");
  useEffect(() => setSearch(""), [kind]);
  const view = useDiscoveryRows({ discovery: w.discovery, kindTab: kind, search, bindingInfo: w.bindingInfo });
  const selection = useDiscoverySelection(view.allRows, w.discovery.scanId, kind);
  return (
    <EditorFrame
      title={t("extensions.discovery.title")}
      backLabel={t("extensions.backToLibrary")}
      busy={w.busy}
      onBack={() => w.nav.setDiscoveryOpen(false)}
      footer={
        <DiscoveryImportBar
          discovery={w.discovery}
          busy={w.writeBlocked}
          selection={selection}
          rows={view.rows}
        />
      }
    >
      {notice}
      <div className="asb-ext-discovery-content">
        <ExtensionSearch kind={kind} search={search} onSearch={setSearch} />
        <DiscoverPanel
          discovery={w.discovery}
          busy={w.writeBlocked}
          kindTab={kind}
          search={search}
          projectNames={w.projectNames}
          bindingInfo={w.bindingInfo}
          view={view}
          selection={selection}
          onViewDetails={(observed) => {
            if (observed.actions.managedDefinitionId)
              w.nav.openManagement(observed.actions.managedDefinitionId, observed.kind);
          }}
          onRepair={(ids) => void w.applies.repair(ids)}
        />
      </div>
    </EditorFrame>
  );
}
