import { useEffect, useState } from "react";
import { getChangelogHistory, type ChangelogEntry } from "../api/client";
import { useI18n } from "../i18n";
import { errorText } from "../i18n/errors";
import { Button } from "./Button";
import { CollapsibleSection } from "./Collapsible";
import { ReleaseNotes } from "./ReleaseNotes";
import { Select } from "./Select";
import { Time } from "./Time";

type HistoryState = { status: "loading" }
  | { status: "ready"; entries: ChangelogEntry[] }
  | { status: "failed"; error: unknown };

function HistoryContents({ appVersion }: { appVersion: string | null }) {
  const { t } = useI18n();
  const [state, setState] = useState<HistoryState>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    setState({ status: "loading" });
    void getChangelogHistory().then((entries) => {
      if (current) setState({ status: "ready", entries });
    }).catch((error: unknown) => {
      if (current) setState({ status: "failed", error });
    });
    return () => { current = false; };
  }, [attempt]);

  if (state.status === "loading") return <p className="asb-field-help" role="status">
    {t("backup.update.historyLoading")}
  </p>;
  if (state.status === "failed") return <>
    <p className="asb-fail-text" role="alert">
      {t("backup.update.historyFailed", { detail: errorText(state.error, t) })}
    </p>
    <div className="asb-app-settings-error-actions">
      <Button variant="secondary" size="compact" onClick={() => setAttempt((value) => value + 1)}>
        {t("backup.update.historyRetry")}
      </Button>
    </div>
  </>;
  const selected = state.entries.find((entry) => entry.version === version)
    ?? state.entries.find((entry) => entry.version === appVersion) ?? state.entries[0];
  if (!selected) return <p className="asb-field-help">{t("backup.update.historyEmpty")}</p>;
  return <>
    <div className="asb-field is-medium">
      <Select value={selected.version} onChange={setVersion} ariaLabel={t("backup.update.historyVersion")}
        options={state.entries.map((entry) => ({ value: entry.version,
          label: t(entry.version === appVersion ? "backup.update.currentVersion" : "backup.update.historyOption",
            { version: entry.version }),
        }))} />
      {selected.date && <span className="asb-field-help"><Time iso={selected.date} mode="date" /></span>}
    </div>
    <ReleaseNotes notes={selected.notes} version={selected.version} />
  </>;
}

/** The shared disclosure mounts the reader on demand and releases it when closed. */
export function ChangelogHistory({ appVersion }: { appVersion: string | null }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <CollapsibleSection className="asb-app-settings-group" open={open} onOpenChange={setOpen}
    label={<span className="asb-section-title">{t("backup.update.historyTitle")}</span>}>
    <HistoryContents appVersion={appVersion} />
  </CollapsibleSection>;
}
