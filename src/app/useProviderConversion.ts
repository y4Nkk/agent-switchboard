import { useCallback, useEffect, useRef, useState } from "react";
import {
  prepareProviderConversion,
  type AppKind, type CodexProviderRecord, type CommandError, type ProviderConversion, type ProviderProfile,
} from "../api/client";
import { createEditorContext } from "./provider-editor-session";
import type { ProviderEditorSession, SetEditorSession } from "./provider-editor-session";
import type { ProvidersDeps } from "./useProviders";

function restoreConversionFocus(id: string) {
  requestAnimationFrame(() => {
    const buttons = document.querySelectorAll<HTMLButtonElement>("[data-provider-conversion-id]");
    [...buttons].find((button) => button.dataset.providerConversionId === id)?.focus();
  });
}

export function useProviderConversion(deps: ProvidersDeps, session: ProviderEditorSession | null, updateSession: SetEditorSession) {
  const [blockedConversion, setBlocked] = useState<ProviderConversion | null>(null);
  const generation = useRef(0);
  const preparing = useRef(false);
  useEffect(() => { generation.current += 1; }, [deps.active, deps.appFilter]);
  useEffect(() => () => { generation.current += 1; }, []);
  const setEditorSession: SetEditorSession = useCallback((value) => {
    generation.current += 1;
    updateSession(value);
  }, [updateSession]);
  const editorSessionVersion = useCallback(() => generation.current, []);
  const dismissConversion = useCallback(() => {
    if (blockedConversion) restoreConversionFocus(blockedConversion.source.id);
    setBlocked(null);
  }, [blockedConversion]);
  const closeEditor = useCallback(() => {
    const snapshot = session?.app === "claude" ? session.conversion
      : session?.source?.kind === "conversion" ? session.source.conversion : null;
    setEditorSession(null);
    if (snapshot) {
      deps.setAppFilter(snapshot.source.app);
      restoreConversionFocus(snapshot.source.id);
    }
  }, [session, setEditorSession, deps.setAppFilter]);
  const convert = useCallback(async (sourceApp: AppKind, sourceProfileId: string, expectedSourceFileHash: string) => {
    if (deps.busy || preparing.current) return;
    preparing.current = true;
    const request = ++generation.current;
    const workspace = deps.workspaceVersion();
    deps.clearCandidates();
    deps.clearError();
    deps.setBusy(true);
    try {
      const targetApp = sourceApp === "codex" ? "claude" : "codex";
      const conversion = await prepareProviderConversion({ sourceApp, sourceProfileId, expectedSourceFileHash, targetApp });
      if (generation.current !== request || workspace !== deps.workspaceVersion()) return;
      if (conversion.status === "blocked" || !conversion.seed) { setBlocked(conversion); return; }
      const sessionId = crypto.randomUUID();
      const context = createEditorContext(sessionId);
      setEditorSession(targetApp === "claude"
        ? { ...context, app: "claude", record: null, conversion, sessionId }
        : { ...context, app: "codex", source: { kind: "conversion", conversion, sessionId } });
      deps.setTargetProfileId(null);
      deps.setAppFilter(targetApp);
    } catch (error) {
      if (generation.current === request && workspace === deps.workspaceVersion()) deps.onError(error as CommandError);
    } finally { preparing.current = false; deps.setBusy(false); }
  }, [deps, setEditorSession]);
  const convertProfile = (profile: ProviderProfile) => {
    const record = [...deps.records, ...deps.codexOfficialRecords].find((item) => item.profile.id === profile.id);
    if (record) void convert(profile.app, profile.id, record.fileHash);
  };
  const convertCodexProfile = (record: CodexProviderRecord) => void convert("codex", record.profile.id, record.fileHash);
  return { setEditorSession, editorSessionVersion, closeEditor, convertProfile, convertCodexProfile, blockedConversion, dismissConversion };
}
