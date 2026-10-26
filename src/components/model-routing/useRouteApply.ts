import { useState } from "react";
import {
  commitCodexProfileSave, commitProfileSave, prepareCodexProfileSave, prepareProfileSave,
} from "../../api/client";
import { uiMessage } from "../../i18n/errors";
import { useProfileSavePreparation } from "../useProfileSavePreparation";
import { routeDocument, type RouteDocument } from "./model";

interface Props {
  active: boolean;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onApplied: () => Promise<void>;
}

export function useRouteApply({ active, busy, onBusy, onApplied }: Props) {
  const [error, setError] = useState<unknown>(null);
  const preparation = useProfileSavePreparation(active, setError);
  const { pending } = preparation;
  const prepare = async (document: RouteDocument) => {
    if (!active || busy || pending) return;
    const request = preparation.checkpoint();
    onBusy(true);
    setError(null);
    try {
      const result = document.kind === "codex"
        ? await prepareCodexProfileSave(document.record.profile.id, document.draft, document.record.fileHash, "activate")
        : await prepareProfileSave(document.record.profile.id, document.draft, document.record.fileHash, "activate");
      const token = { source: document.kind, preparationId: result.preparationId };
      if (!preparation.isCurrent(request)) { await preparation.discard(token); return; }
      if (result.kind !== "saveAndApply" || !result.preview) {
        await preparation.discard(token);
        throw uiMessage("operations.save.previewMissing");
      }
      await preparation.hold({ ...token, app: document.kind === "codex" ? "codex" : document.draft.app, preview: result.preview }, request);
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
        const record = await commitCodexProfileSave(pending.preparationId, true);
        onDocument(routeDocument({ kind: "codex", record }));
      } else {
        const record = await commitProfileSave(pending.preparationId, true);
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
  return { pending, error, prepare, confirm, cancel };
}
