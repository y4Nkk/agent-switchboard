import { useRef, useState } from "react";
import type { ExtensionListItem } from "../../api/client";
import type { SkillDirectoryEntry } from "../../api/extensions/skill-sources";
import {
  skillCandidateInstalled, type SkillSourceActions, type SkillSourceFilter, type SkillSourceKind, type SkillSourceRow,
} from "./skill-source-model";
import { skillRepositoryLabel, skillSourceUrl } from "./skill-repository-model";
import { useSkillSourceCatalog } from "./useSkillSourceCatalog";
import { useSkillSourceDirectory } from "./useSkillSourceDirectory";
import { useSkillSourceFiles } from "./useSkillSourceFiles";
import { useSkillSourceInstall } from "./useSkillSourceInstall";
import { useSkillSourceRepositories } from "./useSkillSourceRepositories";

export type { SkillSourceActions } from "./skill-source-model";

function sourceRows(source: SkillSourceKind, catalog: ReturnType<typeof useSkillSourceCatalog>,
  files: ReturnType<typeof useSkillSourceFiles>): SkillSourceRow[] | null {
  if (source === "catalog") return catalog.result?.candidates.map((candidate) => ({
    candidate, key: JSON.stringify([candidate.repositoryId, candidate.candidateId]),
    repositoryId: candidate.repositoryId, label: skillRepositoryLabel(candidate),
    sourceUrl: skillSourceUrl(candidate.repo, candidate.readmeUrl),
  })) ?? null;
  if (files.result?.kind !== source) return null;
  return files.result.candidates.map((candidate) => ({
    candidate, key: candidate.candidateId, label: files.result!.label, sourceUrl: null,
  }));
}

export function useSkillSource(actions: SkillSourceActions & {
  busy: boolean; items: ExtensionListItem[];
}) {
  const [source, setSource] = useState<SkillSourceKind>("directory");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<SkillSourceFilter>("all");
  const [repository, setRepository] = useState("all");
  const [managerOpen, setManagerOpen] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const repairActive = useRef(false);
  const catalog = useSkillSourceCatalog(actions.busy);
  const directory = useSkillSourceDirectory(actions.busy);
  const files = useSkillSourceFiles(actions);
  const loading = catalog.loading || directory.loading || files.loading;
  const imports = useSkillSourceInstall(actions, actions.items,
    actions.busy || catalog.loading || files.loading || directory.searching);
  const repositories = useSkillSourceRepositories(actions.busy || imports.busy || loading, (next) => {
    catalog.reconcile(next);
    setRepository((current) => next?.some((repo) => repo.id === current) ? current : "all");
  });
  const locked = () => repairActive.current || directory.resolving !== null || imports.isLocked() || (actions.busy && !loading);
  const changeSource = (next: SkillSourceKind) => {
    if (locked() || next === source || (managerOpen && repositories.loading)) return;
    setSource(next); catalog.cancel(); directory.cancel();
    if ((next === "local" || next === "zip") && files.result?.kind !== next) files.clear();
    else files.cancel();
    setQuery(""); setFilter("all"); setRepository("all");
  };
  const search = async () => {
    if (locked()) return;
    if (source === "catalog") {
      if (catalog.retryAt === null && repositories.ready && !repositories.loading) await catalog.refresh(repositories.items);
    } else if (source === "directory") await directory.search();
    else await files.scan(source);
  };
  const repair = async () => {
    if (locked() || loading || repositories.loading || !repositories.ready || catalog.recoveries.length === 0 ||
      catalog.retryAt !== null) return;
    repairActive.current = true;
    setRepairing(true);
    try {
      await catalog.refresh(repositories.items, {
        requests: catalog.recoveries,
        onSaved: (next) => {
          repositories.acceptRecovery(next);
          setRepository((current) => next.some((repo) => repo.id === current) ? current : "all");
        },
      });
    } finally {
      repairActive.current = false;
      setRepairing(false);
    }
  };
  const pick = async () => {
    if (locked() || (source !== "zip" && source !== "local")) return;
    if (await files.pick(source)) { setQuery(""); setFilter("all"); }
  };
  const inspectDirectory = async (entry: SkillDirectoryEntry) => {
    if (!locked()) await directory.resolve(entry);
  };
  const candidates = sourceRows(source, catalog, files);
  const rows = candidates?.map((row) => ({ ...row, installed: skillCandidateInstalled(row.candidate, actions.items) })) ?? [];
  const visible = rows.filter((row) =>
    (repository === "all" || row.repositoryId === repository) &&
    (filter === "all" || (filter === "installed" ? row.installed : !row.installed)) &&
    `${row.candidate.name} ${row.candidate.description ?? ""} ${row.label}`.toLowerCase().includes(query.trim().toLowerCase()));
  return {
    source, query, setQuery, filter, setFilter, repository, setRepository, managerOpen, setManagerOpen,
    catalog, directory, files, repositories, changeSource, search, repair, repairing, pick, inspectDirectory,
    candidates, visible, installedCount: rows.filter((row) => row.installed).length, loading, imports,
    error: source === "catalog" ? catalog.error : source === "directory" ? directory.error : files.error,
  };
}

export type SkillSourceState = ReturnType<typeof useSkillSource>;
