import { uiMessage } from "../../i18n/errors";
import { useCallback, useEffect, useRef, useState } from "react";
import type { SkillCandidateDto } from "../../api/client";
import {
  resolveDirectorySkill, searchSkillDirectory, type SkillDirectoryEntry, type SkillDirectoryResult,
} from "../../api/extensions/skill-sources";
import { useSkillSourceRequests } from "./useSkillSourceRequests";

interface DirectorySnapshot extends SkillDirectoryResult { query: string; nextOffset: number }

export function skillDirectoryKey(entry: SkillDirectoryEntry) {
  return JSON.stringify([entry.id, entry.repo, entry.subpath]);
}

function appendDirectory(previous: DirectorySnapshot, next: SkillDirectoryResult): DirectorySnapshot {
  const items = new Map(previous.items.map((entry) => [skillDirectoryKey(entry), entry]));
  for (const entry of next.items) items.set(skillDirectoryKey(entry), entry);
  return {
    ...next, items: [...items.values()], query: previous.query,
    nextOffset: previous.nextOffset + next.items.length,
    hasMore: next.hasMore && next.items.length > 0,
  };
}

export function useSkillSourceDirectory(busy: boolean) {
  const [input, setInput] = useState("");
  const [resolving, setResolving] = useState<string | null>(null);
  const active = useRef<symbol | null>(null);
  useEffect(() => () => { active.current = null; }, []);
  const searchRequests = useSkillSourceRequests<DirectorySnapshot>(busy);
  const resolution = useSkillSourceRequests<Record<string, SkillCandidateDto[]>>(busy);
  const cancelResolution = () => { active.current = null; setResolving(null); resolution.cancel(); };
  const cancel = () => { searchRequests.cancel(); cancelResolution(); };
  const clearSearch = searchRequests.clear;
  const clearResolution = resolution.clear;
  const clear = useCallback(() => {
    active.current = null; setResolving(null); clearSearch(); clearResolution();
  }, [clearSearch, clearResolution]);
  const changeInput = (value: string) => { setInput(value); cancel(); };
  const search = async () => {
    const query = input.trim();
    if (query.length < 2) { searchRequests.setError(uiMessage("extensions.directory.queryTooShort")); return; }
    cancelResolution();
    const next = await searchRequests.run(async () => {
      const result = await searchSkillDirectory(query, 0);
      return { ...result, query, nextOffset: result.items.length };
    });
    if (next) resolution.clear();
  };
  const loadMore = async () => {
    const previous = searchRequests.result;
    if (!previous?.hasMore || input.trim() !== previous.query || active.current) return;
    await searchRequests.run(async () => appendDirectory(previous,
      await searchSkillDirectory(previous.query, previous.nextOffset)));
  };
  const resolve = async (entry: SkillDirectoryEntry) => {
    if (busy || searchRequests.loading || active.current) return null;
    const key = skillDirectoryKey(entry);
    const known = resolution.result?.[key];
    if (known?.length) return known;
    const token = Symbol();
    active.current = token;
    setResolving(key);
    const next = await resolution.run(async () => ({
      ...resolution.result, [key]: await resolveDirectorySkill(entry),
    }));
    if (active.current === token) { active.current = null; setResolving(null); }
    return next?.[key] ?? null;
  };
  return {
    input, changeInput, search, loadMore, cancel, clear, resolve, resolving,
    result: searchRequests.result, resolutions: resolution.result ?? {},
    searching: searchRequests.loading, loading: searchRequests.loading || resolving !== null,
    error: searchRequests.error, resolveError: resolution.error,
  };
}
