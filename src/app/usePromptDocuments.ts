import { useCallback, useEffect, useRef, useState } from "react";
import {
  getGlobalPromptDocument,
  saveGlobalPromptDocument,
  type AppKind,
  type CommandError,
  type GlobalPromptDocument,
} from "../api/client";

interface PromptDocumentsDeps {
  active: boolean;
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
}

type DocumentMap = Partial<Record<AppKind, GlobalPromptDocument>>;
type DraftMap = Partial<Record<AppKind, string>>;

const SUPPORTED_APPS: readonly AppKind[] = ["codex", "claude"];

/**
 * Owns prompt-management drafts independently from model configuration. Each
 * document has its own request generation so a delayed response can never
 * replace a newer save. The selected client follows the common-settings page.
 */
export function usePromptDocuments({
  active,
  busy,
  onError,
  clearError,
  setBusy,
}: PromptDocumentsDeps) {
  const [documents, setDocuments] = useState<DocumentMap>({});
  const [drafts, setDrafts] = useState<DraftMap>({});
  const documentsRef = useRef<DocumentMap>({});
  const draftsRef = useRef<DraftMap>({});
  const loaded = useRef(new Set<AppKind>());
  const epochs = useRef<Record<AppKind, number>>({ codex: 0, claude: 0 });
  const requests = useRef(new Map<AppKind, Promise<GlobalPromptDocument>>());

  const replaceDocument = useCallback(
    (app: AppKind, document: GlobalPromptDocument, replaceDraft: boolean) => {
      const previous = documentsRef.current[app];
      const previousDraft = draftsRef.current[app] ?? previous?.content ?? "";
      const draftIsDirty = previous !== undefined && previousDraft !== previous.content;
      documentsRef.current = { ...documentsRef.current, [app]: document };
      setDocuments(documentsRef.current);
      if (replaceDraft || !draftIsDirty) {
        draftsRef.current = { ...draftsRef.current, [app]: document.content };
        setDrafts(draftsRef.current);
      }
    },
    [],
  );

  const requestDocument = useCallback((app: AppKind) => {
    const existing = requests.current.get(app);
    if (existing) return existing;
    const request = getGlobalPromptDocument(app);
    requests.current.set(app, request);
    void request
      .finally(() => {
        if (requests.current.get(app) === request) requests.current.delete(app);
      })
      .catch(() => {});
    return request;
  }, []);

  const loadPromptDocument = useCallback(
    async (app: AppKind, replaceDraft = false) => {
      const epoch = ++epochs.current[app];
      try {
        const document = await requestDocument(app);
        if (epochs.current[app] === epoch) replaceDocument(app, document, replaceDraft);
      } catch (caught) {
        if (epochs.current[app] === epoch) onError(caught as CommandError);
        loaded.current.delete(app);
      }
    },
    [onError, replaceDocument, requestDocument],
  );

  useEffect(() => {
    if (!active) return;
    for (const app of SUPPORTED_APPS) {
      if (loaded.current.has(app)) continue;
      loaded.current.add(app);
      void loadPromptDocument(app);
    }
  }, [active, loadPromptDocument]);

  const setPromptDraft = useCallback((app: AppKind, content: string) => {
    draftsRef.current = { ...draftsRef.current, [app]: content };
    setDrafts(draftsRef.current);
  }, []);

  const isPromptDirty = useCallback((app: AppKind) => {
    const document = documentsRef.current[app];
    return document !== undefined && (draftsRef.current[app] ?? document.content) !== document.content;
  }, []);

  const discardPromptDraft = useCallback((app: AppKind) => {
    const document = documentsRef.current[app];
    if (!document) return;
    setPromptDraft(app, document.content);
  }, [setPromptDraft]);

  const reloadPromptDocument = useCallback(
    (app: AppKind) => {
      if (busy || isPromptDirty(app)) return;
      loaded.current.add(app);
      void loadPromptDocument(app, true);
    },
    [busy, isPromptDirty, loadPromptDocument],
  );

  const savePromptDocument = useCallback(
    async (app: AppKind) => {
      if (busy) return;
      const document = documentsRef.current[app];
      if (!document) return;
      const content = draftsRef.current[app] ?? document.content;
      if (content === document.content) return;
      const epoch = ++epochs.current[app];
      setBusy(true);
      clearError();
      try {
        const saved = await saveGlobalPromptDocument(app, content, document.contentHash, true);
        if (epochs.current[app] === epoch) replaceDocument(app, saved, true);
      } catch (caught) {
        if (epochs.current[app] === epoch) onError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, replaceDocument, setBusy],
  );

  return {
    documents,
    drafts,
    setPromptDraft,
    isPromptDirty,
    discardPromptDraft,
    reloadPromptDocument,
    savePromptDocument,
  };
}
