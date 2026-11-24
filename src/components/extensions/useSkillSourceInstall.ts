import { uiMessage } from "../../i18n/errors";
import { useMessageState } from "../../i18n/use-message-state";
import { useEffect, useRef, useState } from "react";
import type { ExtensionListItem, SkillCandidateDto } from "../../api/client";
import { skillCandidateHost, skillCandidateInstalled, sourceErrorMessage, type SkillSourceActions } from "./skill-source-model";

export function useSkillSourceInstall(actions: SkillSourceActions, items: ExtensionListItem[], busy: boolean) {
  const lock = useRef<string | null>(null);
  const latest = useRef({ actions, items, busy });
  latest.current = { actions, items, busy };
  const mounted = useRef(true);
  const [active, setActive] = useState<string | null>(null);
  const [error, setError] = useMessageState();
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  const install = async (candidate: SkillCandidateDto) => {
    const current = latest.current;
    if (!mounted.current || current.busy || lock.current || candidate.diagnostics.length > 0 ||
      skillCandidateInstalled(candidate, current.items)) return;
    lock.current = candidate.candidateId;
    setActive(candidate.candidateId);
    setError(null);
    try {
      const saved = await current.actions.onImport(candidate.candidateId, candidate.name, skillCandidateHost(candidate));
      if (!mounted.current) return;
      if (!saved) setError(uiMessage("extensions.sources.installIncomplete", { name: candidate.name }));
    } catch (reason) {
      lock.current = null;
      if (mounted.current) setError(reason);
    } finally {
      lock.current = null;
      if (mounted.current) setActive(null);
    }
  };
  return { install, active, busy: active !== null, isLocked: () => lock.current !== null, error: error ? sourceErrorMessage(error, "") : null };
}
