import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { CodexProviderDraft, SettingsValues } from "../../api/client";
import type { CodexEditorSource } from "../../app/useProviders";
import { codexDraftFromConversion, conversionInputsComplete, conversionLimitsComplete } from "../../lib/provider-conversion";
import { useI18n } from "../../i18n";
import { useProviderConnection } from "../provider-editor/useProviderConnection";
import { useProviderParameters } from "../provider-editor/useProviderParameters";
import { codexDraftFrom, prepareCodexDraft, validateCodexDraft, type CodexEditorDraft } from "./draft";

function codexDraftFromSource(source: CodexEditorSource | null): CodexEditorDraft {
  if (source?.kind === "record") return codexDraftFrom(source.record);
  if (source?.kind === "conversion" && source.conversion.seed?.app === "codex") {
    return codexDraftFromConversion(source.conversion.seed.draft);
  }
  return codexDraftFrom(null);
}

export function useCodexProviderEditor(source: CodexEditorSource | null, busy: boolean) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<CodexEditorDraft>(() => codexDraftFromSource(source));
  const baseline = useRef(draft);
  const conversion = source?.kind === "conversion" ? source.conversion : null;
  const [reviewedCapabilities, setReviewedCapabilities] = useState<string | null>(null);
  const capabilitySignature = JSON.stringify([draft.capabilities, draft.catalog.map(({ contextWindow: _context, maxOutputTokens: _output, ...entry }) => entry)]);
  const capabilitiesReviewed = reviewedCapabilities === capabilitySignature;
  const setCapabilitiesReviewed = (checked: boolean) => setReviewedCapabilities(checked ? capabilitySignature : null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  // Third-party Codex is always a custom route; Responses is the only upstream
  // that carries request-mode facts.
  const connection = useProviderConnection({
    app: "codex",
    routeMode: "custom",
    baseUrl: draft.endpoint,
    apiKey: draft.apiKey,
    authentication: draft.authentication,
    connection: draft.connection,
    subagentRoute: draft.subagentRoute,
    modelRoutes: draft.modelRoutes,
    upstreamProtocol: draft.upstream,
    responsesOptions: draft.upstream === "responses" ? { requestMode: draft.requestMode } : null,
  });
  const seedParameters = useCallback((defaults: SettingsValues) => {
    if (baseline.current.parameters === null) {
      baseline.current = { ...baseline.current, parameters: { settings: { ...defaults.settings } } };
    }
    setDraft((current) => current.parameters === null
      ? { ...current, parameters: { settings: { ...defaults.settings } } }
      : current);
  }, []);
  const parameters = useProviderParameters("codex", draft.parameters, seedParameters);
  const problems = validateCodexDraft(draft, t);
  const limitsComplete = conversionLimitsComplete(draft);
  const canSave = !busy && !connection.modelsBusy && parameters.ready && problems.length === 0
    && (!conversion || conversionInputsComplete(draft, capabilitiesReviewed));
  const save = (onSave: (value: CodexProviderDraft) => void) => {
    if (!canSave || connection.isModelsPending()) return;
    const value = prepareCodexDraft(draft);
    if (value) onSave(value);
  };
  return { draft, setDraft, dirty: JSON.stringify(draft) !== JSON.stringify(baseline.current),
    headingRef, viewportRef, connection, parameters, problems, canSave, save,
    conversion, capabilitiesReviewed, setCapabilitiesReviewed, limitsComplete };
}

export type CodexEditorState = ReturnType<typeof useCodexProviderEditor>;

/** Narrow helper for section components that patch one catalog entry. */
export type SetCodexDraft = Dispatch<SetStateAction<CodexEditorDraft>>;
