import { LoaderCircle, Plus, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import type { ExtensionListItem } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { EditorFrame } from "../EditorFrame";
import { SearchInput } from "../SearchInput";
import { Select } from "../Select";
import { SearchIcon } from "../icons";
import { ExtensionLoading } from "./ExtensionLoading";
import { SkillDirectoryResults } from "./SkillDirectoryResults";
import { SkillRepositoryManager } from "./SkillRepositoryManager";
import { SkillSourceCandidate } from "./SkillSourceCandidate";
import { SkillSourceForm } from "./SkillSourceForm";
import { sourceErrorMessage, type SkillSourceFilter } from "./skill-source-model";
import { skillRepositoryLabel, skillRepositoryName } from "./skill-repository-model";
import { useSkillSource, type SkillSourceActions, type SkillSourceState } from "./useSkillSource";

interface Props extends SkillSourceActions {
  busy: boolean;
  items: ExtensionListItem[];
  onBack: () => void;
  /** Persistent failure notice rendered at the top of the scroll body. */
  notice?: ReactNode;
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
        {state.loading && t("extensions.sources.rescanning")}
      </p>
      {state.visible.length === 0 ? (
        <div className="asb-skill-source-empty">
          <p>{state.candidates.length === 0 ? (state.source === "catalog" && state.catalog.result?.failures.length ?
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
    {!state.managerOpen && state.repositories.error && <div className="asb-skill-source-error" role="alert">
      <span>{state.repositories.error}</span>
      <Button variant="secondary" disabled={disabled || state.repositories.loading}
        onClick={() => void state.repositories.reload()}><RefreshCw size={16} />{t("extensions.repo.retryLoad")}</Button>
    </div>}
    {state.source === "catalog" && Boolean(state.catalog.result?.failures.length) &&
      <div className="asb-skill-source-error" role="alert">
        <ul className="asb-skill-source-diagnostics">
          {state.catalog.result!.failures.map((failure) => <li key={failure.repositoryId}>
            {t("extensions.sources.failureLine", {
              repo: skillRepositoryName(failure.repo) ?? t("extensions.repo.fallbackName"),
              message: sourceErrorMessage(failure.message, t("extensions.sources.refreshFailed")),
            })}
            {state.catalog.count(failure.repositoryId) !== null && t("extensions.sources.staleResults")}
          </li>)}
        </ul>
        <Button variant="secondary" disabled={disabled || state.repositories.loading} onClick={() => void state.search()}>
          <RefreshCw size={16} />{t("extensions.sources.retryRefresh")}
        </Button>
      </div>}
  </>;
}

/** Finding new skills uses the shared full-page editor frame; the scan
 * action lives in the fixed bottom bar and submits the source form via its id. */
export function SkillSourceBrowser(props: Props) {
  const { t } = useI18n();
  const state = useSkillSource(props);
  const scanning = state.source === "directory" ? state.directory.searching : state.loading;
  const scanDisabled = props.busy || state.loading || state.imports.busy || (state.source === "catalog" &&
    (!state.repositories.ready || state.repositories.loading || !state.repositories.items.some((repo) => repo.enabled)));
  const scanLabel = scanning ? (state.source === "directory" ? t("extensions.sources.searching") : t("extensions.sources.scanning"))
    : state.source === "catalog" ? t("extensions.sources.refreshRepos")
      : state.source === "directory" ? t("extensions.sources.search") : state.source === "zip" ? t("extensions.sources.scanZip") : t("extensions.sources.scanSource");
  return (
    <EditorFrame
      title={t("extensions.sources.title")}
      backLabel={t("extensions.backToLibrary")}
      busy={props.busy}
      onBack={props.onBack}
      footer={
        <Button type="submit" form="skill-source-scan" variant="primary" disabled={scanDisabled}>
          {scanning ? <LoaderCircle size={16} className="asb-skill-source-spinner" />
            : state.source === "catalog" ? <RefreshCw size={16} /> : <SearchIcon />}
          {scanLabel}
        </Button>
      }
    >
      {props.notice}
      <div className="asb-skill-source-browser">
        <SkillSourceForm state={state} busy={props.busy} />
        {state.managerOpen ? <SkillRepositoryManager state={state} busy={props.busy} /> : <>
          <SourceErrors state={state} busy={props.busy} />
          {state.source === "directory" ? <SkillDirectoryResults state={state} busy={props.busy} items={props.items} /> :
            <SourceResults state={state} busy={props.busy} />}
        </>}
      </div>
    </EditorFrame>
  );
}
