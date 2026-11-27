import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";

import { sameClaudeExtra, type ClaudeExtraSettings } from "./claude-common-settings";
import { sameSettingValue } from "../lib/setting-values";
import {
  getClientSettingsEditor,
  getCurrentClientConfiguration,
  onClientConfigChanged,
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
  permissionProfile?: SettingValue;
  keepFileValues?: string[];
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

const automatic: SettingValue = { mode: "automatic" };

export function changedSettingValues(draft: Record<string, SettingValue>, baseline: Record<string, SettingValue>) {
  return Object.fromEntries(Object.entries(draft).filter(([key, value]) =>
    !sameSettingValue(value, baseline[key] ?? automatic)));
}

function changedState(
  state: ClientSettingsEditorState,
  change: Partial<Pick<ClientSettingsEditorState, "draft" | "claudeExtra" | "permissionProfile" | "keepFileValues">>,
): ClientSettingsEditorState {
  const next = { ...state, ...change, error: undefined };
  const dirty = next.permissionProfile !== undefined || !!next.keepFileValues?.length ||
    !sameClaudeExtra(next.claudeExtra, state.editor?.settings.claudeExtra) ||
    Object.keys(changedSettingValues(next.draft ?? {}, state.editor?.source?.values ?? {})).length > 0;
  return { ...next, phase: dirty ? "dirty" : "clean" };
}

function useLoadClientSettings(active: boolean, app: AppKind, states: States, setStates: SetStates) {
  const requests = useRef(new Map<AppKind, Promise<ClientSettingsEditor>>());
  const previousScope = useRef<AppKind | null>(null);
  const load = useCallback((target: AppKind) => {
    const existing = requests.current.get(target);
    if (existing) return existing;
    setStates((current) => ({ ...current, [target]: { ...current[target], phase: "loading", error: undefined } }));
    const request = getClientSettingsEditor(target);
    requests.current.set(target, request);
    void request.then((editor) => setStates((current) => {
      const draft = editor.source?.values ?? {};
      return { ...current, [target]: { ...current[target], phase: "clean", editor, draft,
        claudeExtra: editor.settings.claudeExtra, permissionProfile: undefined, keepFileValues: undefined } };
    })).catch((caught) => setStates((current) => ({
      ...current,
      [target]: { ...current[target], phase: "loadError", error: caught as CommandError },
    }))).finally(() => {
      if (requests.current.get(target) === request) requests.current.delete(target);
    });
    return request;
  }, [setStates]);
  useEffect(() => {
    const scope = active ? app : null;
    const entering = previousScope.current !== scope;
    previousScope.current = scope;
    if (scope && entering && states[scope].phase !== "dirty") void load(scope);
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
  const phase = states[app].phase;
  const currentPhase = useRef(phase);
  currentPhase.current = phase;
  useEffect(() => {
    if (!active || busy || phase !== "clean") return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onClientConfigChanged(() => { if (!disposed && currentPhase.current === "clean") void load(app); }).then((stop) => {
      if (disposed) stop(); else unlisten = stop;
    }).catch(() => {});
    return () => { disposed = true; unlisten?.(); };
  }, [active, app, busy, phase, load]);
  const changeValue = useCallback((target: AppKind, key: string, value: SettingValue) => {
    if (busy) return;
    setStates((current) => {
      const latest = current[target];
      if (!latest.editor?.source || !latest.draft || (latest.phase !== "clean" && latest.phase !== "dirty")) return current;
      return { ...current, [target]: changedState(latest, { draft: { ...latest.draft, [key]: value },
        keepFileValues: latest.keepFileValues?.filter((field) => field !== key) }) };
    });
  }, [busy]);
  const keepFileValue = useCallback((target: AppKind, key: string) => {
    if (busy) return;
    setStates((current) => {
      const latest = current[target];
      if (!latest.editor?.source || latest.editor.settings.settings[key]?.mode !== "explicit"
        || !latest.draft || (latest.phase !== "clean" && latest.phase !== "dirty")) return current;
      const pending = latest.keepFileValues ?? [];
      const keepFileValues = pending.includes(key) ? pending.filter((field) => field !== key) : [...pending, key];
      const draft = { ...latest.draft, [key]: latest.editor.source.values[key] ?? automatic };
      return { ...current, [target]: changedState(latest, { draft, keepFileValues }) };
    });
  }, [busy]);
  const changePermissionProfile = useCallback((value: SettingValue) => {
    if (busy) return;
    setStates((current) => {
      const latest = current.codex;
      if (!latest.editor?.source || (latest.phase !== "clean" && latest.phase !== "dirty")) return current;
      const selected = latest.editor.source.permissionProfile;
      const baseline: SettingValue = selected === null ? automatic : { mode: "explicit", value: selected };
      const permissionProfile = sameSettingValue(value, baseline) ? undefined : value;
      return { ...current, codex: changedState(latest, { permissionProfile }) };
    });
  }, [busy]);
  const changeClaudeExtra = useCallback((target: AppKind, claudeExtra: ClaudeExtraSettings) => {
    if (busy) return;
    if (target !== "claude") return;
    setStates((current) => {
      const latest = current[target];
      if (!latest.editor || !latest.draft) return current;
      return { ...current, [target]: changedState(latest, { claudeExtra }) };
    });
  }, [busy]);
  const retryLoad = useCallback((target: AppKind) => {
    if (!busy) void load(target);
  }, [busy, load]);
  const reloadFromStored = useCallback((target: AppKind) => {
    if (busy) return;
    void load(target);
    reviewConfiguration(target);
  }, [busy, load, reviewConfiguration]);
  return {
    states,
    changeValue,
    keepFileValue,
    changePermissionProfile,
    changeClaudeExtra,
    retryLoad,
    reloadFromStored,
    reviewConfiguration,
  };
}
