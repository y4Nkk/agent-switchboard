import type { AppKind, CommandError, LocalizedMessage } from "../api/client";
import type { ProvidersDeps } from "./useProviders";

export interface ConversionSource {
  id: string;
  name: string;
  fileHash: string;
  model: string | null;
  endpoint: string | null;
}

export interface ConversionSelection {
  phase: "selecting";
  sourceApp: AppKind;
  sources: ConversionSource[];
}

interface ConversionProgress {
  sourceApp: AppKind;
  sources: ConversionSource[];
  index: number;
  saved: number;
  skipped: number;
}

export type ConversionBatch = ConversionProgress & (
  | { phase: "ready" | "refreshing" | "preparing" | "complete" }
  | { phase: "editing" | "saving"; editorId: string }
  | { phase: "blocked"; reason: LocalizedMessage }
  | { phase: "error"; error: CommandError });

export type ConversionFlow = ConversionSelection | ConversionBatch;

/** Hooks report persistence milestones; they do not own or advance the queue. */
export interface ConversionSaveLifecycle {
  begin: (profileId: string) => void;
  complete: (profileId: string) => boolean;
  finish: (profileId: string) => void;
}

export function conversionSources(deps: Pick<ProvidersDeps, "codexRecords" | "codexOfficialRecords" | "records">, app: AppKind): ConversionSource[] {
  const generic = [...deps.records, ...deps.codexOfficialRecords].filter(({ profile }) => profile.app === app)
    .map(({ profile, fileHash, position }) => ({ id: profile.id, name: profile.name, fileHash, position,
      model: profile.model, endpoint: profile.baseUrl }));
  const codex = app === "codex" ? deps.codexRecords.map(({ profile, fileHash, position }) => ({
    id: profile.id, name: profile.name, fileHash, position, model: profile.defaultModel, endpoint: profile.endpoint,
  })) : [];
  return [...generic, ...codex].sort((a, b) => a.position - b.position || a.id.localeCompare(b.id))
    .map(({ position: _position, ...source }) => source);
}

export function conversionProgress(batch: ConversionBatch): ConversionProgress {
  const { sourceApp, sources, index, saved, skipped } = batch;
  return { sourceApp, sources, index, saved, skipped };
}

export function advanceConversion(batch: ConversionBatch, result: "saved" | "skipped"): ConversionBatch {
  const index = batch.index + 1;
  return { sourceApp: batch.sourceApp, sources: batch.sources, index,
    saved: batch.saved + Number(result === "saved"), skipped: batch.skipped + Number(result === "skipped"),
    phase: index === batch.sources.length ? "complete" : "ready" };
}
