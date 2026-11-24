import { useCallback, useState } from "react";
import {
  scanSkillRepositories, type SkillCatalogResult, type SkillRepository, type SkillRepositoryScanProgress,
} from "../../api/extensions/skill-sources";
import { sameSkillRepository } from "./skill-repository-model";
import { useSkillSourceRequests } from "./useSkillSourceRequests";

interface CatalogSnapshot extends SkillCatalogResult { scanned: SkillRepository[] }

function filterCatalog(snapshot: SkillCatalogResult, repositories: SkillRepository[]): SkillCatalogResult {
  const rows = snapshot.progress.repositories.filter(({ repository }) =>
    repositories.some((current) => sameSkillRepository(current, repository)));
  const allowed = new Set(rows.map(({ repository }) => repository.id));
  return {
    candidates: snapshot.candidates.filter((row) => allowed.has(row.repositoryId)),
    progress: { ...snapshot.progress, repositories: rows },
  };
}

function mergeCatalog(next: SkillCatalogResult, previous: CatalogSnapshot | null, repositories: SkillRepository[]) {
  const fresh = filterCatalog(next, repositories);
  const failed = fresh.progress.repositories.filter((row) => row.state === "failed");
  const retained = previous?.scanned.filter((repo) =>
    failed.some(({ repository }) => sameSkillRepository(repository, repo))) ?? [];
  const retainedIds = new Set(retained.map((repo) => repo.id));
  const completed = fresh.progress.repositories.filter((row) => row.state === "complete").map((row) => row.repository);
  return {
    ...fresh,
    candidates: [...fresh.candidates, ...(previous?.candidates.filter((row) => retainedIds.has(row.repositoryId)) ?? [])],
    scanned: [...completed, ...retained],
  };
}

const INITIAL_PROGRESS = { snapshot: null, unavailable: false };

export function useSkillSourceCatalog(busy: boolean) {
  const requests = useSkillSourceRequests<CatalogSnapshot>(busy);
  const [live, setLive] = useState<{ snapshot: SkillRepositoryScanProgress | null; unavailable: boolean }>(INITIAL_PROGRESS);
  const cancel = useCallback(() => {
    requests.cancel();
    setLive(INITIAL_PROGRESS);
  }, [requests.cancel]);
  const refresh = async (repositories: SkillRepository[]) => {
    await requests.run(async (isCurrent) => {
      setLive(INITIAL_PROGRESS);
      try {
        const next = await scanSkillRepositories(isCurrent, (snapshot) => setLive((previous) =>
          snapshot ? { snapshot, unavailable: false } : { ...previous, unavailable: true }));
        return mergeCatalog(next, requests.result, repositories);
      } finally {
        if (isCurrent()) setLive(INITIAL_PROGRESS);
      }
    });
  };
  const reconcile = (repositories: SkillRepository[] | null) => {
    cancel();
    requests.update((previous) => {
      if (!previous || !repositories) return null;
      const current = filterCatalog(previous, repositories);
      if (current.progress.repositories.length === 0) return null;
      return {
        ...current,
        scanned: previous.scanned.filter((repo) => repositories.some((next) => sameSkillRepository(next, repo))),
      };
    });
  };
  const count = (id: string) => requests.result?.scanned.some((repo) => repo.id === id) ?
    requests.result.candidates.filter((candidate) => candidate.repositoryId === id).length : null;
  const failedRepositoryIds = new Set(requests.result?.progress.repositories
    .filter((row) => row.state === "failed").map((row) => row.repository.id));
  return {
    ...requests, cancel, refresh, reconcile, count, failedRepositoryIds,
    progress: requests.loading ? live.snapshot : requests.result?.progress ?? null,
    progressUnavailable: requests.loading && live.unavailable,
  };
}
