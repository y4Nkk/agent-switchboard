import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";

import { sameClaudeExtra, type ClaudeExtraSettings } from "./claude-common-settings";
import { sameSettingValue } from "../lib/setting-values";
import {
  getClientSettingsEditor,
  getCurrentClientConfiguration,
  type AppKind,
  type ClientSettingsEditor,
  type CommandError,
  type CurrentClientConfiguration,
  type SettingValue,
} from "../api/client";

export type ClientSettingsPhase = "idle" | "loading" | "loadError" | "clean" | "dirty";

export interface ClientSettingsEditorState {
  phase: ClientSettingsPhase;
  editor?: ClientSettingsEditor;
  draft?: Record<string, SettingValue>;
  claudeExtra?: ClaudeExtraSettings;
  error?: CommandError;
  currentConfiguration?: CurrentClientConfiguration;
  currentConfigurationLoading?: boolean;
  currentConfigurationError?: CommandError;
}

interface ClientSettingsDeps {
  app: AppKind;
  busy: boolean;
  active: boolean;
}

type States = Record<AppKind, ClientSettingsEditorState>;
type SetStates = Dispatch<SetStateAction<States>>;

function sameValues(left: Record<string, SettingValue>, right: Record<string, SettingValue>): boolean {
  return Object.keys(left).length === Object.keys(right).length && Object.keys(left).every((key) => {
    const before = left[key];
    const after = right[key];
    return after !== undefined && sameSettingValue(before, after);
  });
}

function phaseForDraft(
  editor: ClientSettingsEditor,
  draft: Record<string, SettingValue>,
  extra: ClaudeExtraSettings,
): ClientSettingsPhase {
  return !sameValues(draft, editor.settings.settings) || !sameClaudeExtra(extra, editor.settings.claudeExtra)
    ? "dirty" : "clean";
}

function changedState(
  state: ClientSettingsEditorState,
  draft: Record<string, SettingValue>,
  claudeExtra: ClaudeExtraSettings = state.claudeExtra,
): ClientSettingsEditorState {
  return {
    ...state,
    draft,
    claudeExtra,
    phase: phaseForDraft(state.editor!, draft, claudeExtra),
    error: undefined,
  };
}

function useLoadClientSettings(active: boolean, app: AppKind, states: States, setStates: SetStates) {
  const requests = useRef(new Map<AppKind, Promise<ClientSettingsEditor>>());
  const load = useCallback((target: AppKind, preserveDraft = false) => {
    const existing = requests.current.get(target);
    if (existing) return existing;
    setStates((current) => ({ ...current, [target]: { ...current[target], phase: "loading", error: undefined } }));
    const request = getClientSettingsEditor(target);
    requests.current.set(target, request);
    void request.then((editor) => setStates((current) => {
      const prior = current[target];
      const draft = preserveDraft && prior.draft ? prior.draft : editor.settings.settings;
      const claudeExtra = preserveDraft && prior.draft ? prior.claudeExtra : editor.settings.claudeExtra;
      return { ...current, [target]: { phase: phaseForDraft(editor, draft, claudeExtra), editor, draft, claudeExtra } };
    })).catch((caught) => setStates((current) => ({
      ...current,
      [target]: { ...current[target], phase: "loadError", error: caught as CommandError },
    }))).finally(() => {
      if (requests.current.get(target) === request) requests.current.delete(target);
    });
    return request;
  }, [setStates]);
  useEffect(() => {
    if (active && states[app].phase === "idle") void load(app);
  }, [active, app, load, states]);
  return load;
}

function useCurrentClientConfiguration(busy: boolean, setStates: SetStates) {
  const revisions = useRef<Record<AppKind, number>>({ codex: 0, claude: 0 });
  return useCallback((app: AppKind) => {
    if (busy) return;
    revisions.current[app] += 1;
    const revision = revisions.current[app];
    setStates((current) => ({
      ...current,
      [app]: {
        ...current[app],
        currentConfigurationLoading: true,
        currentConfigurationError: undefined,
      },
    }));
    void getCurrentClientConfiguration(app).then((currentConfiguration) => setStates((current) => {
      if (revisions.current[app] !== revision) return current;
      return { ...current, [app]: { ...current[app], currentConfiguration, currentConfigurationLoading: false } };
    })).catch((caught) => setStates((current) => {
      if (revisions.current[app] !== revision) return current;
      return {
        ...current,
        [app]: { ...current[app], currentConfigurationLoading: false, currentConfigurationError: caught as CommandError },
      };
    }));
  }, [busy, setStates]);
}

/** Each client preference draft lives in application state until its explicit save. */
export function useClientSettings(deps: ClientSettingsDeps) {
  const { active, app, busy } = deps;
  const [states, setStates] = useState<States>({ codex: { phase: "idle" }, claude: { phase: "idle" } });
  const load = useLoadClientSettings(active, app, states, setStates);
  const reviewConfiguration = useCurrentClientConfiguration(busy, setStates);
  const changeValue = useCallback((target: AppKind, key: string, value: SettingValue) => {
    if (busy) return;
    const state = states[target];
    if (!state.editor || !state.draft) return;
    const draft = { ...state.draft, [key]: value };
    setStates((current) => {
      const latest = current[target];
      if (!latest.editor || !latest.draft) return current;
      return { ...current, [target]: changedState(latest, draft) };
    });
  }, [busy, setStates, states]);
  const changeClaudeExtra = useCallback((target: AppKind, claudeExtra: ClaudeExtraSettings) => {
    if (busy) return;
    const state = states[target];
    if (target !== "claude" || !state.editor || !state.draft) return;
    setStates((current) => {
      const latest = current[target];
      if (!latest.editor || !latest.draft) return current;
      return { ...current, [target]: changedState(latest, latest.draft, claudeExtra) };
    });
  }, [busy, setStates, states]);
  const retryLoad = useCallback((target: AppKind) => {
    if (!busy) void load(target, states[target].draft !== undefined);
  }, [busy, load, states]);
  const reloadFromStored = useCallback((target: AppKind) => {
    if (busy) return;
    void load(target);
    reviewConfiguration(target);
  }, [busy, load, reviewConfiguration]);
  return {
    states,
    changeValue,
    changeClaudeExtra,
    retryLoad,
    reloadFromStored,
    reviewConfiguration,
  };
}
