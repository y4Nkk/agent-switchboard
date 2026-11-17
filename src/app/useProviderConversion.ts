import { useCallback, useEffect, useRef, useState } from "react";
import { listCodexProfiles, listProfiles, prepareProviderConversion, type AppKind, type CommandError } from "../api/client";
import { uiMessage } from "../i18n/errors";
import { createEditorContext, type ProviderEditorSession, type SetEditorSession } from "./provider-editor-session";
import { advanceConversion, conversionProgress, conversionSources, type ConversionBatch, type ConversionFlow,
  type ConversionSaveLifecycle, type ConversionSource } from "./provider-conversion-batch";
import type { ProvidersDeps } from "./useProviders";

function restoreConversionFocus(app: AppKind) {
  requestAnimationFrame(() => document.querySelector<HTMLButtonElement>(`[data-provider-conversion-app="${app}"]`)?.focus());
}

async function readConversionSource(app: AppKind, id: string): Promise<ConversionSource> {
  const [records, codexRecords] = await Promise.all([listProfiles(), app === "codex" ? listCodexProfiles() : Promise.resolve([])]);
  const source = conversionSources({ records, codexRecords, codexOfficialRecords: [] }, app).find((item) => item.id === id);
  if (!source) throw uiMessage("providers.conversion.error.sourceMissing");
  return source;
}

function useConversionFlow(deps: ProvidersDeps) {
  const [state, setState] = useState<ConversionFlow | null>(null);
  const current = useRef(state);
  const update = useCallback((next: ConversionFlow | null) => { current.current = next; setState(next); }, []);
  const start = (ids: string[]) => {
    const selection = current.current;
    if (!deps.active || deps.busy || selection?.phase !== "selecting" || selection.sourceApp !== deps.appFilter) return;
    const selected = new Set(ids);
    const sources = selection.sources.filter(({ id }) => selected.has(id));
    if (!sources.length) return;
    update({ sourceApp: selection.sourceApp, sources, index: 0, saved: 0, skipped: 0, phase: "ready" });
  };
  const retry = async () => {
    const batch = current.current;
    if (batch?.phase !== "error" || deps.busy) return;
    const pending: ConversionBatch = { ...conversionProgress(batch), phase: "refreshing" };
    const workspace = deps.workspaceVersion();
    update(pending);
    deps.setBusy(true);
    try {
      const source = await readConversionSource(batch.sourceApp, batch.sources[batch.index].id);
      if (current.current === pending && workspace === deps.workspaceVersion()) update({ ...conversionProgress(batch),
        sources: batch.sources.map((item, index) => index === batch.index ? source : item), phase: "ready" });
    } catch (error) {
      if (current.current === pending) update({ ...conversionProgress(batch), phase: "error", error: error as CommandError });
    } finally {
      if (current.current === pending) update({ ...conversionProgress(batch), phase: "error", error: uiMessage("modelRoutes.contextChanged") });
      deps.setBusy(false);
    }
  };
  return { state, current, update, start, retry };
}

type Flow = ReturnType<typeof useConversionFlow>;

function useConversionPreparation(deps: ProvidersDeps, session: ProviderEditorSession | null, flow: Flow,
  generation: { current: number }, updateSession: SetEditorSession) {
  useEffect(() => {
    const batch = flow.current.current;
    if (!deps.active || deps.busy || session || batch?.phase !== "ready") return;
    const pending: ConversionBatch = { ...conversionProgress(batch), phase: "preparing" };
    flow.update(pending);
    const request = ++generation.current;
    const workspace = deps.workspaceVersion();
    const isCurrent = () => generation.current === request && workspace === deps.workspaceVersion() && flow.current.current === pending;
    deps.clearCandidates();
    deps.clearError();
    deps.setBusy(true);
    const prepare = async () => {
      try {
        const source = batch.sources[batch.index];
        const targetApp = batch.sourceApp === "codex" ? "claude" : "codex";
        const conversion = await prepareProviderConversion({ sourceApp: batch.sourceApp, sourceProfileId: source.id,
          expectedSourceFileHash: source.fileHash, targetApp });
        if (!isCurrent()) return;
        if (conversion.status === "blocked" && conversion.blockedReason) {
          flow.update({ ...conversionProgress(batch), phase: "blocked", reason: conversion.blockedReason });
          return;
        }
        if (!conversion.seed || conversion.seed.app !== targetApp || conversion.status === "blocked")
          throw uiMessage("providers.conversion.error.invalidResult");
        const sessionId = crypto.randomUUID();
        const context = createEditorContext(sessionId);
        generation.current += 1;
        updateSession(targetApp === "claude"
          ? { ...context, app: "claude", record: null, conversion, sessionId }
          : { ...context, app: "codex", source: { kind: "conversion", conversion, sessionId } });
        flow.update({ ...conversionProgress(batch), phase: "editing", editorId: sessionId });
        deps.setTargetProfileId(null);
        deps.setAppFilter(targetApp);
      } catch (error) {
        if (isCurrent()) flow.update({ ...conversionProgress(batch), phase: "error", error: error as CommandError });
      } finally {
        if (flow.current.current === pending) flow.update({ ...conversionProgress(batch), phase: "error", error: uiMessage("modelRoutes.contextChanged") });
        deps.setBusy(false);
      }
    };
    void prepare();
  }, [deps, session, flow, generation, updateSession]);
}

function useConversionSaves(flow: Flow, generation: { current: number }, updateSession: SetEditorSession): ConversionSaveLifecycle {
  return {
    begin: (profileId) => {
      const batch = flow.current.current;
      if (batch?.phase === "editing" && batch.editorId === profileId) flow.update({ ...batch, phase: "saving" });
    },
    complete: (profileId) => {
      const batch = flow.current.current;
      if (batch?.phase !== "saving" || batch.editorId !== profileId) return false;
      generation.current += 1;
      updateSession(null);
      flow.update(advanceConversion(batch, "saved"));
      return true;
    },
    finish: (profileId) => {
      const batch = flow.current.current;
      if (batch?.phase === "saving" && batch.editorId === profileId) flow.update({ ...batch, phase: "editing" });
    },
  };
}

export function useProviderConversion(deps: ProvidersDeps, session: ProviderEditorSession | null, updateSession: SetEditorSession) {
  const flow = useConversionFlow(deps);
  const generation = useRef(0);
  useEffect(() => { generation.current += 1; }, [deps.active, deps.appFilter]);
  useEffect(() => () => { generation.current += 1; }, []);
  const setEditorSession: SetEditorSession = useCallback((value) => {
    generation.current += 1;
    if (value !== null) flow.update(null);
    updateSession(value);
  }, [updateSession, flow.update]);
  const editorSessionVersion = useCallback(() => generation.current, []);
  const cancelConversion = useCallback(() => {
    const state = flow.current.current;
    generation.current += 1;
    flow.update(null);
    if (state?.phase === "editing" || state?.phase === "saving") updateSession(null);
  }, [flow.update, updateSession]);
  const endConversion = () => {
    const state = flow.current.current;
    if (deps.busy || state?.phase === "saving" || state?.phase === "preparing" || state?.phase === "refreshing") return;
    cancelConversion();
    if (state && state.phase !== "complete") deps.setAppFilter(state.sourceApp);
    restoreConversionFocus(state && state.phase !== "complete" ? state.sourceApp : deps.appFilter);
  };
  const closeEditor = () => {
    if (flow.current.current) endConversion();
    else setEditorSession(null);
  };
  const skipConversion = () => {
    const batch = flow.current.current;
    if (deps.busy || !batch || batch.phase === "selecting" || !["editing", "blocked", "error"].includes(batch.phase)) return;
    generation.current += 1;
    updateSession(null);
    flow.update(advanceConversion(batch, "skipped"));
  };
  const openConversion = () => {
    if (deps.active && !deps.busy && !session && !flow.current.current)
      flow.update({ phase: "selecting", sourceApp: deps.appFilter, sources: conversionSources(deps, deps.appFilter) });
  };
  const conversionSave = useConversionSaves(flow, generation, updateSession);
  useConversionPreparation(deps, session, flow, generation, updateSession);
  return { setEditorSession, editorSessionVersion, closeEditor, openConversion, endConversion, cancelConversion, skipConversion,
    retryConversion: flow.retry, conversionSave, startConversion: flow.start,
    conversionSelection: flow.state?.phase === "selecting" ? flow.state : null,
    conversionBatch: flow.state && flow.state.phase !== "selecting" ? flow.state : null };
}
