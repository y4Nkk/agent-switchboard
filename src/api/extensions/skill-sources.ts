import { invoke } from "../client";
import type { SkillCandidateDto } from "./types";

export interface SkillRepository {
  id: string;
  repo: string;
  subpath: string;
  refName: string | null;
  enabled: boolean;
}

export type SkillRepositoryInput = Omit<SkillRepository, "id"> & { id?: string };

export interface SkillCatalogCandidate extends SkillCandidateDto {
  repositoryId: string;
  repo: string;
  subpath: string;
  refName: string | null;
  readmeUrl: string | null;
}

export interface SkillCatalogResult {
  candidates: SkillCatalogCandidate[];
  progress: SkillRepositoryScanProgress;
}

export type SkillRepositoryRequestStep = "commit" | "tree" | "archive";

export interface SkillRepositoryRequest {
  step: SkillRepositoryRequestStep;
  state: "waiting" | "reading" | "received" | "failed";
  httpStatus: number | null;
  receivedBytes: number;
  totalBytes: number | null;
}

export type SkillRepositoryScanRow = {
  repository: SkillRepository;
  requests: SkillRepositoryRequest[];
} & (
  | { state: "queued" | "running" }
  | { state: "complete"; candidateCount: number }
  | { state: "failed"; message: string }
);

export interface SkillRepositoryScanProgress {
  elapsedMs: number;
  repositories: SkillRepositoryScanRow[];
}

export interface SkillDirectoryEntry {
  id: string;
  name: string;
  repo: string;
  /** Directory search may return a slug; resolution finds its actual source path. */
  subpath: string;
  installs: number;
  readmeUrl: string | null;
}

export function listSkillRepositories(): Promise<SkillRepository[]> {
  return invoke<SkillRepository[]>("list_skill_repositories");
}

export function saveSkillRepository(input: SkillRepositoryInput): Promise<SkillRepository> {
  return invoke<SkillRepository>("save_skill_repository", { input });
}

export function removeSkillRepository(id: string): Promise<void> {
  return invoke<void>("remove_skill_repository", { id });
}

/** Explicit network refresh. Saving or listing repositories never fetches them. */
export async function scanSkillRepositories(
  isCurrent: () => boolean,
  onProgress: (progress: SkillRepositoryScanProgress | null) => void,
): Promise<SkillCatalogResult> {
  const idBytes = crypto.getRandomValues(new Uint8Array(16));
  const scanId = Array.from(idBytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  let finished = false;
  let readFailed = false;
  let timer: number | undefined;
  const observing = () => !finished && isCurrent();
  const read = async () => {
    if (!observing()) return;
    try {
      const snapshot = await invoke<SkillRepositoryScanProgress | null>("read_skill_repository_scan", { scanId });
      if (snapshot && observing()) {
        readFailed = false;
        onProgress(snapshot);
      }
    } catch (error) {
      if (observing() && !readFailed) {
        readFailed = true;
        console.warn("Could not read repository scan progress", error);
        onProgress(null);
      }
    }
    if (observing()) timer = window.setTimeout(() => void read(), 300);
  };

  const result = invoke<SkillCatalogResult>("scan_skill_repositories", { scanId });
  void read();
  try {
    return await result;
  } finally {
    finished = true;
    window.clearTimeout(timer);
  }
}

export function searchSkillDirectory(query: string): Promise<SkillDirectoryEntry[]> {
  return invoke("search_skill_directory", { query });
}

export function resolveDirectorySkill(entry: SkillDirectoryEntry): Promise<SkillCandidateDto[]> {
  return invoke<SkillCandidateDto[]>("resolve_directory_skill", { entry });
}

/** Reads candidates only; import and deployment use the existing library/executor. */
export function scanSkillZip(path: string): Promise<SkillCandidateDto[]> {
  return invoke<SkillCandidateDto[]>("scan_skill_zip", { path });
}
