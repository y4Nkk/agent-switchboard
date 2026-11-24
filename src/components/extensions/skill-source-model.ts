import type { AppKind, ExtensionListItem, ExtensionMutation, SkillCandidateDto } from "../../api/client";
import { tr } from "../../i18n/current";

export interface SkillSourceActions {
  onScanLocal: (root: string) => Promise<SkillCandidateDto[] | null>;
  onImport: (candidateId: string, name: string, host: AppKind | null) => Promise<ExtensionMutation | null>;
  onPickDirectory: () => Promise<string | null>;
  onPickZip: () => Promise<string | null>;
}

export type SkillSourceKind = "catalog" | "directory" | "zip" | "local";
export type SkillSourceFilter = "all" | "installed" | "pending";
export interface SkillSourceResult {
  kind: "local" | "zip";
  candidates: SkillCandidateDto[];
  label: string;
}
export interface SkillSourceRow {
  key: string;
  candidate: SkillCandidateDto;
  label: string;
  sourceUrl: string | null;
  repositoryId?: string;
}

export function skillCandidateHost(candidate: SkillCandidateDto): AppKind | null {
  return candidate.description?.trim() ? null : "claude";
}

export function skillCandidateInstalled(candidate: SkillCandidateDto, items: ExtensionListItem[]) {
  const clients: AppKind[] = skillCandidateHost(candidate) ? ["claude"] : ["codex", "claude"];
  return clients.every((client) => items.some((item) =>
    item.kind === "skill" && item.contentDigest === candidate.digest &&
    (item.hostScoped === null || item.hostScoped === client) && item.bindings.some((binding) =>
      binding.target.scope === "app" && binding.target.client === client &&
      binding.desired === "enabled" && binding.fileState === "inSync" &&
      (!binding.lockedDigest || binding.lockedDigest === candidate.digest),
    ),
  ));
}

export function skillSourceHasOtherVersion(candidate: SkillCandidateDto, items: ExtensionListItem[]) {
  const source = candidate.source;
  if (!source?.repo) return false;
  const repo = source.repo.toLowerCase();
  return items.some((item) => item.kind === "skill" && item.contentDigest !== candidate.digest &&
    item.source?.repo?.toLowerCase() === repo && item.source?.subpath === source.subpath &&
    (item.source?.refName ?? null) === (source.refName ?? null));
}

export function sourceErrorMessage(error: unknown, fallback: string) {
  const message = typeof error === "string" ? error :
    error && typeof error === "object" && "message" in error && typeof error.message === "string" ? error.message : "";
  return (message.trim() || fallback).replace(/https?:\/\/[^\s<>"']+/gi, (value) => {
    try {
      const url = new URL(value);
      return `${url.protocol}//${url.host}${url.pathname}`;
    } catch { return tr("extensions.sources.urlHidden"); }
  });
}
