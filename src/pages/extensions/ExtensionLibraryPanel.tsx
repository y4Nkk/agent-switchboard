import { Button } from "../../components/Button";
import { ExtensionLoading } from "../../components/extensions/ExtensionLoading";
import { SearchIcon } from "../../components/icons";
import { useI18n } from "../../i18n";
import { Server, Sparkles } from "lucide-react";
import { ExtensionList } from "./ExtensionList";
import { ScrollArea } from "../../components/ScrollArea";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

/** The library's empty state: the one shape from panels.css, with the heading
 * and the action saying which kind of empty this is — nothing added yet, or a
 * filter that matched nothing. */
function EmptyLibrary({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const empty = workspace.kindItems.length === 0;
  const { nav } = workspace;
  return (
    <div className="asb-empty-state">
      <span className="asb-empty-state-icon" aria-hidden="true">
        {empty ? nav.kind === "skill" ? <Sparkles /> : <Server /> : <SearchIcon />}
      </span>
      <h3 className="asb-section-title">
        {empty
          ? t(nav.kind === "skill" ? "extensions.library.emptySkills" : "extensions.library.emptyMcp")
          : t("extensions.library.noFilterMatches")}
      </h3>
      {!empty && (
        <Button variant="secondary" onClick={nav.clearFilters}>
          {t("common.clearSearch")}
        </Button>
      )}
    </div>
  );
}

function LibraryState({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  if (!workspace.ext.loaded) return <ExtensionLoading />;
  if (!workspace.ext.workspace)
    return (
      <div className="asb-empty-state" role="alert">
        <h3 className="asb-section-title">{t("extensions.library.loadFailed")}</h3>
        <Button
          variant="secondary"
          disabled={workspace.busy}
          onClick={() => void workspace.ext.runExclusive(workspace.ext.refresh)}
        >
          {t("extensions.library.reload")}
        </Button>
      </div>
    );
  return <EmptyLibrary workspace={workspace} />;
}

export function ExtensionLibraryPanel({ workspace }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  if (workspace.nav.kind === null) return null;
  const loaded = workspace.ext.loaded && workspace.ext.workspace !== null;
  const filtered = loaded && workspace.visible.length !== workspace.kindItems.length;
  return (
    <section className="asb-ext-library-panel" aria-label={t("extensions.library.aria")}>
      {loaded && workspace.visible.length > 0 ? (
        <ExtensionList workspace={workspace} />
      ) : (
        <ScrollArea className="asb-ext-library-state" role="region" aria-label={t("extensions.library.aria")} tabIndex={0}>
          <LibraryState workspace={workspace} />
        </ScrollArea>
      )}
      {filtered && (
        <p className="asb-ext-result-count" role="status">{t("extensions.library.showing", { count: workspace.visible.length })}</p>
      )}
    </section>
  );
}
