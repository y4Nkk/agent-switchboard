import { useCallback, useEffect, useRef, useState } from "react";
import type { AppKind, ProviderDraft, ProviderProfile, SettingsValues } from "../../api/client";
import { draftFrom, prepareDraft, responsesOptionsValid } from "./draft";
import { useProviderConnection } from "./useProviderConnection";
import { useProviderParameters } from "./useProviderParameters";

export function useProviderEditor(profile: ProviderProfile | null, initialApp: AppKind, busy: boolean) {
  const [draft, setDraft] = useState(() => draftFrom(profile, initialApp));
  const baseline = useRef(draft);
  const [parametersOpen, setParametersOpen] = useState(false);
  const [loginDone, setLoginDone] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const lastSection = useRef(false);
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
  useEffect(() => {
    if (parametersOpen) {
      viewportRef.current?.scrollTo({ top: 0 });
      headingRef.current?.focus();
    } else if (lastSection.current) triggerRef.current?.focus();
    lastSection.current = parametersOpen;
  }, [parametersOpen]);
  const canSave = !busy && parameters.ready && responsesOptionsValid(draft) &&
    (draft.routeMode !== "official" || Boolean(profile) || loginDone);
  const save = (onSave: (value: ProviderDraft) => void) => {
    if (!canSave) return;
    const value = prepareDraft(draft);
    if (value) onSave(value);
  };
  return { draft, setDraft, dirty: JSON.stringify(draft) !== JSON.stringify(baseline.current), parametersOpen, setParametersOpen, loginDone, setLoginDone,
    triggerRef, headingRef, viewportRef, connection, parameters, canSave, save };
}

export type ProviderEditorState = ReturnType<typeof useProviderEditor>;
