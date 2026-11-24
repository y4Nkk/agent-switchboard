import { uiMessage } from "../../i18n/errors";
import { useEffect, useRef, useState } from "react";
import type { SkillCandidateDto } from "../../api/client";
import {
  resolveDirectorySkill, searchSkillDirectory, type SkillDirectoryEntry,
} from "../../api/extensions/skill-sources";
import { useSkillSourceRequests } from "./useSkillSourceRequests";

interface DirectorySnapshot { query: string; items: SkillDirectoryEntry[] }

export function skillDirectoryKey(entry: SkillDirectoryEntry) {
  return JSON.stringify([entry.id, entry.repo, entry.subpath]);
}

export function useSkillSourceDirectory(busy: boolean) {
  const [input, setInput] = useState("");
  const [page, setPage] = useState(1);
  const [resolving, setResolving] = useState<string | null>(null);
  const active = useRef<symbol | null>(null);
  useEffect(() => () => { active.current = null; }, []);
  const searchRequests = useSkillSourceRequests<DirectorySnapshot>(busy);
  const resolution = useSkillSourceRequests<Record<string, SkillCandidateDto[]>>(busy);
  const cancelResolution = () => { active.current = null; setResolving(null); resolution.cancel(); };
  const cancel = () => { searchRequests.cancel(); cancelResolution(); };
  const changeInput = (value: string) => { setInput(value); cancel(); };
  const search = async () => {
    const query = input.trim();
    if (query.length < 2) { searchRequests.setError(uiMessage("extensions.directory.queryTooShort")); return; }
    cancelResolution();
    const next = await searchRequests.run(async (isCurrent) => {
      const items = await searchSkillDirectory(query);
      if (isCurrent()) setPage(1);
      return { query, items };
    });
    if (next) resolution.clear();
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
    input, changeInput, search, page, setPage, cancel, resolve, resolving,
    result: searchRequests.result, resolutions: resolution.result ?? {},
    searching: searchRequests.loading, loading: searchRequests.loading || resolving !== null,
    error: searchRequests.error, resolveError: resolution.error,
  };
}
