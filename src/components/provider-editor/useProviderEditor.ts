import { useCallback, useEffect, useRef, useState } from "react";
import type { AppKind, ProviderDraft, ProviderProfile, SettingsValues } from "../../api/client";
import { draftFrom, prepareDraft, responsesOptionsValid } from "./draft";
import { useProviderConnection } from "./useProviderConnection";
import { useProviderParameters } from "./useProviderParameters";
import { providerDraftFromConversion } from "../../lib/provider-conversion";

export function useProviderEditor(profile: ProviderProfile | null, initialApp: AppKind, busy: boolean, seed?: ProviderDraft) {
  const [draft, setDraft] = useState(() => seed ? providerDraftFromConversion(seed) : draftFrom(profile, initialApp));
  const baseline = useRef(draft);
  const savedModelsText = draft.modelOptions?.kind === "claude" ? (draft.modelOptions.availableModels ?? []).join("\n") : "";
  const [availableModelsText, setAvailableModelsText] = useState(savedModelsText);
  useEffect(() => {
    setAvailableModelsText((current) => current.split("\n").map((id) => id.trim()).filter(Boolean).join("\n") === savedModelsText ? current : savedModelsText);
  }, [savedModelsText]);
  const [loginDone, setLoginDone] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const connection = useProviderConnection(draft);
  const app = draft.app;
  const seedParameters = useCallback((defaults: SettingsValues) => {
    if (baseline.current.app === app && baseline.current.parameters === null) {
      baseline.current = { ...baseline.current, parameters: { settings: { ...defaults.settings } } };
    }
    setDraft((current) => current.app === app && current.parameters === null
      ? { ...current, parameters: { settings: { ...defaults.settings } } }
      : current);
  }, [app]);
  const parameters = useProviderParameters(app, draft.parameters, seedParameters);
  const canSave = !busy && !connection.modelsBusy && parameters.ready && responsesOptionsValid(draft) &&
    (draft.routeMode !== "official" || Boolean(profile) || loginDone);
  const save = (onSave: (value: ProviderDraft) => void) => {
    if (!canSave || connection.isModelsPending()) return;
    const value = prepareDraft(draft);
    if (value) onSave(value);
  };
  return { draft, setDraft, dirty: JSON.stringify(draft) !== JSON.stringify(baseline.current), loginDone, setLoginDone,
    headingRef, viewportRef, connection, parameters, availableModelsText, setAvailableModelsText, canSave, save };
}

export type ProviderEditorState = ReturnType<typeof useProviderEditor>;
