import { useEffect, useRef, useState } from "react";
import { chooseClaudeSessionSource, forgetClaudeSessionSource, listClaudeSessionSources, type ClaudeSessionSource } from "../../api/sessions";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";

export function SessionSourcesDialog({ onClose, onChanged }: { onClose: () => void; onChanged: () => void }) {
  const { t } = useI18n();
  const [sources, setSources] = useState<ClaudeSessionSource[]>([]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useMessageState();
  const locked = useRef(false);
  useEffect(() => {
    let current = true;
    void listClaudeSessionSources().then((sources) => { if (current) setSources(sources); })
      .catch((error: unknown) => { if (current) setError(error); })
      .finally(() => { if (current) setBusy(false); });
    return () => { current = false; };
  }, [setError]);
  const change = async (path?: string) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(null);
    try {
      const next = path === undefined ? await chooseClaudeSessionSource() : await forgetClaudeSessionSource(path);
      if (next !== null) { setSources(next); onChanged(); }
    } catch (error) { setError(error); }
    finally { locked.current = false; setBusy(false); }
  };
  return <AppDialog title={t("sessions.sources.title")} busy={busy} onClose={onClose}
    footer={<Button variant="secondary" disabled={busy} onClick={() => void change()}>{t("sessions.sources.add")}</Button>}>
    <p className="asb-scope-note">{t("sessions.sources.note")}</p>
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    {busy && <p className="asb-scope-note" role="status">{t("sessions.sources.busy")}</p>}
    <ul className="asb-session-sources">
      {sources.map((source) => <li key={source.path}>
        <div><p className="asb-fact-path">{source.path}</p>
          <p className="asb-scope-note">{t(source.available ? "sessions.sources.available" : "sessions.sources.unavailable")}</p>
          {source.issue && <p className="asb-warn-text" role="alert">{source.issue}</p>}</div>
        {source.removable && <Button variant="secondary" disabled={busy} onClick={() => void change(source.path)}>
          {t("sessions.sources.forget")}</Button>}
      </li>)}
    </ul>
  </AppDialog>;
}
