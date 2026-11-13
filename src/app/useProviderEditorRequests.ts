import { useCallback, useEffect, useRef, useState } from "react";
import type { AppKind, CodexProviderRecord, ProviderProfile } from "../api/client";
import type { RouteRole } from "../components/model-routing/model";
import { createEditorContext, type ProviderEditorRequest, type ProviderEditorSession, type SetEditorSession } from "./provider-editor-session";
import type { ProvidersDeps } from "./useProviders";

export function useProviderEditorRequests(deps: ProvidersDeps, session: ProviderEditorSession | null, setSession: SetEditorSession) {
  const [request, setRequest] = useState<ProviderEditorRequest | null>(null);
  const revision = useRef(0);
  const liveDeps = useRef(deps);
  liveDeps.current = deps;
  const handled = useCallback((value: number) => {
    if (request?.revision === value && !request.reload && session?.profileId === request.profileId && session.app === request.app) {
      setSession((current) => current?.profileId === request.profileId && current.app === request.app
        ? { ...current, section: request.section, role: request.role, returnTo: request.returnTo } : current);
    }
    setRequest((current) => current?.revision === value ? null : current);
  }, [request, session, setSession]);
  const replace = useCallback((next: ProviderEditorRequest) => {
    const latest = liveDeps.current;
    const context = { ...createEditorContext(next.profileId), section: next.section, role: next.role, returnTo: next.returnTo };
    const codex = latest.codexRecords.find(({ profile }) => profile.id === next.profileId);
    const generic = [...latest.records, ...latest.codexOfficialRecords].find(({ profile }) => profile.id === next.profileId && profile.app === next.app);
    if (next.app === "codex" && codex) setSession({ ...context, app: "codex", source: { kind: "record", record: codex } });
    else if (next.app === "codex" && generic) setSession({ ...context, app: "codex", source: { kind: "official", record: generic } });
    else if (next.app === "claude" && generic) setSession({ ...context, app: "claude", record: generic });
    else {
      latest.onError({ code: "profile-not-found", message: "", messageKey: "modelRoutes.missingTarget" });
      handled(next.revision);
      return;
    }
    latest.clearCandidates();
    latest.setTargetProfileId(next.profileId);
    latest.setAppFilter(next.app);
    latest.openEditorWorkspace();
    handled(next.revision);
  }, [handled, setSession]);
  const open = useCallback((app: AppKind, profileId: string, section: ProviderEditorRequest["section"], role?: RouteRole,
    returnTo: ProviderEditorRequest["returnTo"] = "providers") => {
    setRequest({ app, profileId, section, role, returnTo, revision: ++revision.current, reload: false });
  }, []);
  const reloadEditor = (section: ProviderEditorRequest["section"], role: RouteRole) => {
    if (session) setRequest({ app: session.app, profileId: session.profileId, section, role,
      returnTo: session.returnTo, revision: ++revision.current, reload: true });
  };
  useEffect(() => {
    if (!request || deps.busy) return;
    if (!session) { replace(request); return; }
    deps.setAppFilter(session.app);
    deps.openEditorWorkspace();
  }, [request, session, deps.busy, deps.setAppFilter, deps.openEditorWorkspace, replace]);
  return { editorRequest: request, handleEditorRequest: handled, replaceEditorSession: replace, reloadEditor,
    openEditor: (profile: ProviderProfile) => open(profile.app, profile.id, "connection"),
    openCodexEditor: (record: CodexProviderRecord) => open("codex", record.profile.id, "connection"),
    openRoutes: (app: AppKind, id: string, role?: RouteRole, returnTo?: ProviderEditorRequest["returnTo"]) => open(app, id, "models", role, returnTo) };
}
