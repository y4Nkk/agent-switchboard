import { useCallback, useEffect, useState } from "react";
import {
  scanSkillRepositories, type SkillCatalogResult, type SkillRepository, type SkillRepositoryScanProgress,
  type SkillRepositoryFailedRow, type SkillRepositoryRecoveryRequest,
} from "../../api/extensions/skill-sources";
import { sameSkillRepository } from "./skill-repository-model";
import { useSkillSourceRequests } from "./useSkillSourceRequests";

type CatalogContents = Pick<SkillCatalogResult, "candidates" | "progress">;
interface CatalogSnapshot extends CatalogContents { scanned: SkillRepository[] }
interface CatalogRecovery {
  requests: SkillRepositoryRecoveryRequest[];
  onSaved: (repositories: SkillRepository[]) => void;
}

function filterCatalog(snapshot: CatalogContents, repositories: SkillRepository[]): CatalogContents {
  const rows = snapshot.progress.repositories.filter(({ repository }) =>
    repositories.some((current) => sameSkillRepository(current, repository)));
  const allowed = new Set(rows.map(({ repository }) => repository.id));
  return {
    candidates: snapshot.candidates.filter((row) => allowed.has(row.repositoryId)),
    progress: { ...snapshot.progress, repositories: rows },
  };
}

function mergeCatalog(next: CatalogContents, previous: CatalogSnapshot | null, repositories: SkillRepository[]) {
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

function useActiveRetryTime(retryAt: number | null) {
  const [now, setNow] = useState(Date.now);
  const pending = retryAt !== null && retryAt * 1000 > now;
  useEffect(() => {
    if (retryAt === null || !pending) return;
    const remaining = Math.max(0, retryAt * 1000 - Date.now());
    const timer = window.setTimeout(() => setNow(Date.now()), Math.min(remaining, 2_147_483_647));
    return () => window.clearTimeout(timer);
  }, [retryAt, pending, now]);
  return pending ? retryAt : null;
}

export function useSkillSourceCatalog(busy: boolean) {
  const requests = useSkillSourceRequests<CatalogSnapshot>(busy);
  const [live, setLive] = useState<{ snapshot: SkillRepositoryScanProgress | null; unavailable: boolean }>(INITIAL_PROGRESS);
  const cancel = useCallback(() => {
    requests.cancel();
    setLive(INITIAL_PROGRESS);
  }, [requests.cancel]);
  const refresh = (repositories: SkillRepository[], recovery?: CatalogRecovery) =>
    requests.run(async (isCurrent) => {
      setLive(INITIAL_PROGRESS);
      try {
        const next = await scanSkillRepositories(isCurrent, (snapshot) => setLive((previous) =>
          snapshot ? { snapshot, unavailable: false } : { ...previous, unavailable: true }), recovery?.requests ?? []);
        if (isCurrent() && next.updatedRepositories) recovery?.onSaved(next.updatedRepositories);
        return mergeCatalog(next, requests.result, next.updatedRepositories ?? repositories);
      } finally {
        if (isCurrent()) setLive(INITIAL_PROGRESS);
      }
    });
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
  const failures = new Map<string, SkillRepositoryFailedRow>();
  const recoveries: SkillRepositoryRecoveryRequest[] = [];
  for (const row of requests.result?.progress.repositories ?? []) {
    if (row.state !== "failed") continue;
    failures.set(row.repository.id, row);
    if (row.recovery) recoveries.push({ repository: row.repository, recovery: row.recovery });
  }
  const progress = requests.loading ? live.snapshot : requests.result?.progress ?? null;
  const retryAt = useActiveRetryTime(progress?.githubRetryAt ?? null);
  return {
    ...requests, cancel, refresh, reconcile, count, failures, recoveries, retryAt, progress,
    progressUnavailable: requests.loading && live.unavailable,
  };
}
