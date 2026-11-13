import { useCallback, useState } from "react";
import type { AppKind, ProviderRecord } from "../api/client";
import type { ProvidersDeps } from "./useProviders";
import { createEditorContext, type ProviderEditorSession } from "./provider-editor-session";
import { useProviderConversion } from "./useProviderConversion";
import { useProviderEditorRequests } from "./useProviderEditorRequests";
import { restoreProviderEditorFocus } from "./provider-editor-focus";

export function useProviderEditorSession(deps: ProvidersDeps) {
  const { appFilter, clearCandidates, setAppFilter, setTargetProfileId } = deps;
  const [editorSession, updateSession] = useState<ProviderEditorSession | null>(null);
  const conversion = useProviderConversion(deps, editorSession, updateSession);
  const { setEditorSession } = conversion;
  const stored = useProviderEditorRequests(deps, editorSession, setEditorSession);
  /** Switches an open Codex editor between its two access modes by replacing
   * the session; a third-party draft never mutates into an official record. */
  const switchCodexAccessMode = useCallback(
    (official: boolean, existing: ProviderRecord | null) => {
      clearCandidates();
      setTargetProfileId(existing?.profile.id ?? null);
      setEditorSession({
        ...createEditorContext(existing?.profile.id),
        app: "codex",
        source: official
          ? { kind: "official", record: existing }
          : null,
      });
    },
    [clearCandidates, setTargetProfileId],
  );
  const newEditor = useCallback(() => {
    clearCandidates();
    setEditorSession(
      appFilter === "codex"
        ? { ...createEditorContext(), app: "codex", source: null }
        : { ...createEditorContext(), app: "claude", record: null },
    );
  }, [appFilter, clearCandidates]);
  /** Cross-contract client switch: replace the session instead of mutating
   * one contract's draft into the other's. */
  const newEditorFor = useCallback((app: AppKind) => {
    clearCandidates();
    setAppFilter(app);
    setTargetProfileId(null);
    setEditorSession(
      app === "codex"
        ? { ...createEditorContext(), app: "codex", source: null }
        : { ...createEditorContext(), app: "claude", record: null },
    );
  }, [clearCandidates, setAppFilter, setTargetProfileId]);
  const closeEditor = () => {
    const destination = editorSession?.returnTo;
    conversion.closeEditor();
    if (destination === "gateway") deps.returnToGateway();
    else if (editorSession && !(editorSession.app === "claude" ? editorSession.conversion : editorSession.source?.kind === "conversion")) {
      restoreProviderEditorFocus(editorSession.profileId, editorSession.section);
    }
  };
  const openCodexOfficialEditor = (record: ProviderRecord | null) => {
    if (record) { stored.openRoutes("codex", record.profile.id, "main"); return; }
    clearCandidates();
    setAppFilter("codex");
    setEditorSession({ ...createEditorContext(), app: "codex", source: { kind: "official", record: null } });
  };
  /** Changing the list client clears the execution target and both preview states, not the editor session. */
  const selectApp = useCallback(
    (app: AppKind) => {
      clearCandidates();
      setAppFilter(app);
      setTargetProfileId(null);
    },
    [clearCandidates, setAppFilter, setTargetProfileId],
  );
  return { editorSession, setEditorSession, editorSessionVersion: conversion.editorSessionVersion, ...stored, openCodexOfficialEditor,
    switchCodexAccessMode, newEditor, newEditorFor, closeEditor, selectApp,
    convertProfile: conversion.convertProfile, convertCodexProfile: conversion.convertCodexProfile,
    blockedConversion: conversion.blockedConversion, dismissConversion: conversion.dismissConversion };
}
