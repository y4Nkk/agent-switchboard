import { Plus, RefreshCw } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ExtensionListItem } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { ScrollArea } from "../ScrollArea";
import { SearchInput } from "../SearchInput";
import { Select } from "../Select";
import { SearchIcon } from "../icons";
import { ExtensionLoading } from "./ExtensionLoading";
import { SkillDirectoryResults } from "./SkillDirectoryResults";
import { SkillRepositoryManager } from "./SkillRepositoryManager";
import { SkillRepositoryScanPanel } from "./SkillRepositoryScanPanel";
import { SkillSourceCandidate } from "./SkillSourceCandidate";
import { SkillSourceForm } from "./SkillSourceForm";
import type { SkillSourceFilter } from "./skill-source-model";
import { skillRepositoryLabel } from "./skill-repository-model";
import { useSkillSource, type SkillSourceActions, type SkillSourceState } from "./useSkillSource";

interface Props extends SkillSourceActions {
  busy: boolean;
  items: ExtensionListItem[];
}

function SourceFilters({ state }: { state: SkillSourceState }) {
  const { t } = useI18n();
  return <div className="asb-skill-source-toolbar">
    <div className="asb-skill-source-search">
      <SearchInput placeholder={t("extensions.sources.searchPlaceholder")} ariaLabel={t("extensions.sources.filterAria")}
        value={state.query} onChange={state.setQuery} />
    </div>
    {state.source === "catalog" && <Select value={state.repository} onChange={state.setRepository}
      ariaLabel={t("extensions.sources.repoFilterAria")} options={[
        { value: "all", label: t("extensions.sources.allRepos") },
        ...state.repositories.items.map((repo) => ({ value: repo.id, label: skillRepositoryLabel(repo) })),
      ]} />}
    <Select value={state.filter} onChange={(value) => state.setFilter(value as SkillSourceFilter)}
      ariaLabel={t("extensions.sources.statusFilterAria")} options={[
        { value: "all", label: t("extensions.sources.allStatuses") }, { value: "installed", label: t("extensions.sources.installed") }, { value: "pending", label: t("extensions.sources.pending") },
      ]} />
  </div>;
}

function SourceResults({ state, busy }: { state: SkillSourceState; busy: boolean }) {
  const { t } = useI18n();
  const noRepositories = state.source === "catalog" && state.repositories.ready && state.repositories.items.length === 0;
  if (noRepositories) return <div className="asb-skill-source-empty" role="status">
    <SearchIcon /><p>{t("extensions.repo.none")}</p>
    <Button variant="secondary" disabled={busy || state.imports.busy} onClick={() => state.setManagerOpen(true)}>
      <Plus size={16} />{t("extensions.repo.add")}
    </Button>
  </div>;
  if (state.candidates === null && state.source === "catalog" && state.catalog.loading) return null;
  if (state.candidates === null) return state.loading ? (
    <section className="asb-skill-source-results" aria-label={t("extensions.sources.resultsAria")} aria-busy="true">
      <ExtensionLoading />
    </section>
  ) : (
    <div className="asb-skill-source-empty" role="status">
      <SearchIcon />
      <p>{state.source === "catalog" ? t("extensions.sources.notRefreshed") : t("extensions.sources.notScanned")}</p>
    </div>
  );
  return (
    <section className="asb-skill-source-results" aria-label={t("extensions.sources.resultsAria")} aria-busy={state.loading}>
      <SourceFilters state={state} />
      <p className="asb-skill-source-count asb-num" role="status">
        {t("extensions.sources.count", {
          visible: state.visible.length,
          total: state.candidates.length,
          installed: state.installedCount,
        })}
        {state.loading && state.source !== "catalog" && t("extensions.sources.rescanning")}
      </p>
      {state.visible.length === 0 ? (
        <div className="asb-skill-source-empty">
          <p>{state.candidates.length === 0 ? (state.source === "catalog" && state.catalog.failedRepositoryIds.size > 0 ?
            t("extensions.sources.noCandidates") : t("extensions.sources.noSkillMd")) : t("extensions.sources.noFilterMatches")}</p>
          {state.candidates.length > 0 && <Button variant="secondary" onClick={() => {
            state.setQuery(""); state.setFilter("all"); state.setRepository("all");
          }}>{t("extensions.sources.clearFilters")}</Button>}
        </div>
      ) : <ul className="asb-skill-source-grid">
        {state.visible.map((row) => <SkillSourceCandidate key={row.key} row={row} installed={row.installed}
          imports={state.imports} source={state.source} busy={busy || state.loading} />)}
      </ul>}
    </section>
  );
}

function SourceErrors({ state, busy }: { state: SkillSourceState; busy: boolean }) {
  const { t } = useI18n();
  const disabled = busy || state.loading || state.imports.busy;
  return <>
    {state.error && <p className="asb-warn-text" role="alert">{state.error}</p>}
    {state.imports.error && <p className="asb-warn-text" role="alert">{state.imports.error}</p>}
    {state.source === "directory" && state.directory.resolveError &&
      <p className="asb-warn-text" role="alert">{state.directory.resolveError}</p>}
    {state.source === "catalog" && state.repositories.error && <div className="asb-skill-source-error" role="alert">
      <span>{state.repositories.error}</span>
      <Button variant="secondary" disabled={disabled || state.repositories.loading}
        onClick={() => void state.repositories.reload()}><RefreshCw size={16} />{t("extensions.repo.retryLoad")}</Button>
    </div>}
  </>;
}

export function SkillSourceBrowser(props: Props) {
  const { t } = useI18n();
  const state = useSkillSource(props);
  const results = useRef<HTMLDivElement>(null);
  const manager = useRef<HTMLDivElement>(null);
  const wasManaging = useRef(false);
  useEffect(() => {
    if (wasManaging.current !== state.managerOpen) {
      (state.managerOpen ? manager : results).current?.focus({ preventScroll: true });
      wasManaging.current = state.managerOpen;
    }
  }, [state.managerOpen]);
  return (
    <div className="asb-ext-market">
      {!state.managerOpen && <SkillSourceForm state={state} busy={props.busy} />}
      <ScrollArea className="asb-ext-market-results" role="region" aria-label={t("extensions.registry.marketSkill")}
        viewportRef={results} tabIndex={0} hidden={state.managerOpen}>
        <div className="asb-skill-source-browser">
          <SourceErrors state={state} busy={props.busy} />
          {state.source === "catalog" && (state.catalog.loading || state.catalog.progress) &&
            <SkillRepositoryScanPanel progress={state.catalog.progress} running={state.catalog.loading}
              unavailable={state.catalog.progressUnavailable} hasPrevious={(id) => state.catalog.count(id) !== null} />}
          {state.source === "directory" ? <SkillDirectoryResults state={state} busy={props.busy} items={props.items} /> :
            <SourceResults state={state} busy={props.busy} />}
        </div>
      </ScrollArea>
      {state.managerOpen &&
        <ScrollArea className="asb-ext-market-results" role="region" aria-label={t("extensions.repo.title")}
          viewportRef={manager} tabIndex={0}>
          <SkillRepositoryManager state={state} busy={props.busy} />
        </ScrollArea>}
    </div>
  );
}
