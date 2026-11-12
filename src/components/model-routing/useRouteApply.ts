import { useEffect, useRef, useState } from "react";
import {
  commitCodexProfileSave, commitProfileSave, prepareCodexProfileSave, prepareProfileSave,
  type ProfileSaveKind, type FilePreview,
} from "../../api/client";
import { uiMessage } from "../../i18n/errors";
import { useProfileSavePreparation, type ProfileSaveToken } from "../useProfileSavePreparation";
import { routeDocument, routeDescription, routeRoles, type RouteDocument, type RouteInventory } from "./model";

interface Props {
  inventory: RouteInventory;
  active: boolean;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onApplied: () => Promise<void>;
}

export interface PendingRouteSave extends ProfileSaveToken {
  kind: ProfileSaveKind;
  preview: FilePreview | null;
  document: RouteDocument;
  currentName: string | null;
  changes: { role: string; before: ReturnType<typeof routeDescription>; after: ReturnType<typeof routeDescription> }[];
}

export function useRouteApply({ inventory, active, busy, onBusy, onApplied }: Props) {
  const [error, setError] = useState<unknown>(null);
  const context = JSON.stringify([inventory.activeIds, ...inventory.codex.map((r) => [r.profile.id, r.fileHash]),
    ...inventory.profiles.map((r) => [r.profile.id, r.fileHash])]);
  const previousContext = useRef(context);
  const preparation = useProfileSavePreparation<PendingRouteSave>(active, setError, context);
  const { pending } = preparation;
  useEffect(() => {
    if (previousContext.current !== context && pending) setError(uiMessage("modelRoutes.contextChanged"));
    previousContext.current = context;
  }, [context, pending]);
  const prepare = async (document: RouteDocument) => {
    if (!active || busy || pending) return;
    const request = preparation.checkpoint();
    onBusy(true);
    setError(null);
    try {
      await preparation.retryReleases();
      if (!preparation.isCurrent(request)) return;
      const result = document.kind === "codex"
        ? await prepareCodexProfileSave(document.record.profile.id, document.draft, document.record.fileHash)
        : await prepareProfileSave(document.record.profile.id, document.draft, document.record.fileHash);
      const token = { source: document.kind, preparationId: result.preparationId };
      if (!preparation.isCurrent(request)) { await preparation.discard(token); return; }
      if (result.kind === "saveAndApply" && !result.preview) {
        await preparation.discard(token);
        throw uiMessage("operations.save.previewMissing");
      }
      const app = document.kind === "codex" ? "codex" : document.draft.app;
      const currentName = [...inventory.codex, ...inventory.profiles]
        .find(({ profile }) => profile.id === inventory.activeIds[app])?.profile.name ?? null;
      const original = routeDocument(document);
      const changes = routeRoles(document).map((role) => ({ role,
        before: routeDescription(original, role, inventory), after: routeDescription(document, role, inventory) }))
        .filter(({ before, after }) => JSON.stringify(before) !== JSON.stringify(after));
      await preparation.hold({ ...token, app, kind: result.kind, document, currentName, changes, preview: result.preview }, request);
    } catch (cause) { if (preparation.isCurrent(request)) setError(cause); }
    finally { onBusy(false); }
  };
  const confirm = async (onDocument: (document: RouteDocument) => void) => {
    if (busy) return;
    const pending = preparation.take();
    if (!pending) return;
    onBusy(true);
    setError(null);
    try {
      if (pending.source === "codex") {
        const record = await commitCodexProfileSave(pending.preparationId, pending.kind === "saveAndApply");
        onDocument(routeDocument({ kind: "codex", record }));
      } else {
        const record = await commitProfileSave(pending.preparationId, pending.kind === "saveAndApply");
        onDocument(routeDocument({ kind: "profile", record }));
      }
      await onApplied();
    } catch (cause) { setError(cause); }
    finally { preparation.finish(); onBusy(false); }
  };
  const cancel = async () => {
    if (busy) return;
    onBusy(true);
    setError(null);
    try { await preparation.cancel(); }
    catch (cause) { setError(cause); }
    finally { onBusy(false); }
  };
  return { pending, error, prepare, confirm, cancel, releaseError: preparation.releaseError };
}
