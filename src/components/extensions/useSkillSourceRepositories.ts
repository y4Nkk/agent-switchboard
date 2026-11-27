import { useEffect, useState } from "react";
import {
  listSkillRepositories, removeSkillRepository, saveSkillRepository,
  type SkillRepository, type SkillRepositoryInput,
} from "../../api/extensions/skill-sources";
import { useSkillSourceRequests } from "./useSkillSourceRequests";

export function useSkillSourceRepositories(busy: boolean, onChanged: (repos: SkillRepository[] | null) => void) {
  const requests = useSkillSourceRequests<SkillRepository[]>(false);
  const [saving, setSaving] = useState(false);
  const reload = async () => {
    const repositories = await requests.run(() => listSkillRepositories());
    if (repositories) onChanged(repositories);
  };
  useEffect(() => { void reload(); }, []);

  const mutate = async (write: (previous: SkillRepository[]) => Promise<SkillRepository[]>) => {
    if (busy || !requests.result) return false;
    const previous = requests.result;
    const repositories = await requests.run(async () => {
      setSaving(true);
      try {
        return await write(previous);
      } finally {
        setSaving(false);
      }
    });
    if (repositories) onChanged(repositories);
    return repositories !== null;
  };
  return {
    items: requests.result ?? [], loading: requests.loading, saving, error: requests.error,
    ready: requests.result !== null, reload,
    acceptRecovery: (repositories: SkillRepository[]) => requests.update(() => repositories),
    save: (input: SkillRepositoryInput) => mutate(async (previous) => {
      const saved = await saveSkillRepository(input);
      return previous.some((repo) => repo.id === saved.id) ?
        previous.map((repo) => repo.id === saved.id ? saved : repo) : [...previous, saved];
    }),
    remove: (id: string) => mutate(async (previous) => {
      await removeSkillRepository(id);
      return previous.filter((repo) => repo.id !== id);
    }),
  };
}
