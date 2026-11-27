import { FolderGit2, LoaderCircle } from "lucide-react";
import type { ExtensionListItem } from "../../api/client";
import type { SkillDirectoryEntry } from "../../api/extensions/skill-sources";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Pagination } from "../Pagination";
import { SearchIcon } from "../icons";
import { ExtensionMarketResult } from "./ExtensionMarket";
import { ExtensionLoading } from "./ExtensionLoading";
import { skillCandidateInstalled, skillSourceHasOtherVersion } from "./skill-source-model";
import { skillRepositoryLabel, skillSourceUrl } from "./skill-repository-model";
import { SkillSourceCandidate, SkillSourceLink } from "./SkillSourceCandidate";
import type { SkillSourceState } from "./useSkillSource";
import { skillDirectoryKey } from "./useSkillSourceDirectory";

const PAGE_SIZE = 20;

function DirectoryEntry({ entry, state, busy }: { entry: SkillDirectoryEntry; state: SkillSourceState; busy: boolean }) {
  const { t } = useI18n();
  const key = skillDirectoryKey(entry);
  const resolving = state.directory.resolving === key;
  const empty = state.directory.resolutions[key]?.length === 0;
  const actionLabel = resolving ? t("extensions.sources.installing") : empty ? t("extensions.sources.retry") : t("extensions.registry.inspectSkill");
  return (
    <ExtensionMarketResult title={entry.name}
      origin={<p className="asb-skill-source-origin"><FolderGit2 size={14} /><span>{skillRepositoryLabel(entry)}</span></p>}
      actions={<>
        <SkillSourceLink url={skillSourceUrl(entry.repo, entry.readmeUrl)} label={t("extensions.sources.viewSourceAria", { name: entry.name })} />
        <Button variant="primary" disabled={busy || state.loading || state.imports.busy}
          aria-label={`${actionLabel} ${entry.name}`}
          onClick={() => void state.inspectDirectory(entry)}>
          {resolving ? <LoaderCircle size={16} className="asb-skill-source-spinner" /> : <SearchIcon />}
          {actionLabel}
        </Button>
      </>}>
      <span className="asb-skill-source-meta asb-num">{t("extensions.sources.installs", { count: entry.installs.toLocaleString() })}</span>
      {empty && <p className="asb-warn-text">{t("extensions.directory.noSkillMd")}</p>}
    </ExtensionMarketResult>
  );
}

export function SkillDirectoryResults({ state, busy, items }: {
  state: SkillSourceState; busy: boolean; items: ExtensionListItem[];
}) {
  const { t } = useI18n();
  const { directory } = state;
  if (!directory.result) return directory.searching ? (
    <section className="asb-skill-source-results" aria-label={t("extensions.directory.resultsAria")} aria-busy="true">
      <ExtensionLoading />
    </section>
  ) : (
    <div className="asb-skill-source-empty" role="status">
      <SearchIcon />
      <p>{t("extensions.directory.notSearched")}</p>
    </div>
  );
  const { items: results, query } = directory.result;
  const first = (directory.page - 1) * PAGE_SIZE;
  return (
    <section className="asb-skill-source-results" aria-label={t("extensions.directory.resultsAria")} aria-busy={directory.loading}>
      <p className="asb-skill-source-count asb-num" role="status">
        {t("extensions.directory.count", {
          query, count: results.length,
        })}
      </p>
      {results.length === 0 ? <div className="asb-skill-source-empty"><p>{t("extensions.directory.noMatches")}</p></div> :
        <ul className="asb-ext-market-list">
          {results.slice(first, first + PAGE_SIZE).flatMap((entry) => {
            const key = skillDirectoryKey(entry);
            const candidates = directory.resolutions[key];
            return candidates?.length ? candidates.map((candidate) => (
              <SkillSourceCandidate key={`${key}:${candidate.candidateId}`} row={{
                key, candidate, label: skillRepositoryLabel(entry), sourceUrl: candidate.source?.readmeUrl ?? skillSourceUrl(entry.repo, entry.readmeUrl),
              }} installed={skillCandidateInstalled(candidate, items)} imports={state.imports} source="directory"
                hasOtherVersion={skillSourceHasOtherVersion(candidate, items)} busy={busy || state.loading} installs={entry.installs} />
            )) : [<DirectoryEntry key={key} entry={entry} state={state} busy={busy} />];
          })}
        </ul>}
      <Pagination total={results.length} page={directory.page} pageSize={PAGE_SIZE} onPageChange={directory.setPage} />
    </section>
  );
}
