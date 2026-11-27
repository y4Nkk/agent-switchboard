import { useRef, useState } from "react";
import { inspectClientConfigurationSources, pickDirectory, type AppKind, type CommandError,
  type ConfigurationObservation, type ConfigurationSources } from "../api/client";
import { useI18n, type TFunction } from "../i18n";
import { catalogText, commandErrorText } from "../i18n/errors";
import { AppDialog } from "./AppDialog";
import { Button } from "./Button";
import { FactList } from "./FactList";

export function useClientConfigurationSources(app: AppKind) {
  const [report, setReport] = useState<ConfigurationSources | null>(null);
  const [directory, setDirectory] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);
  const request = useRef(false);
  const run = async (chooseDirectory: boolean, userScope: boolean) => {
    if (request.current) return;
    request.current = true;
    setBusy(true);
    setError(null);
    try {
      const next = chooseDirectory ? await pickDirectory() : userScope ? null : directory;
      if (chooseDirectory && next === null) return;
      setDirectory(next);
      setReport(null);
      const inspected = await inspectClientConfigurationSources(app, next);
      setDirectory(inspected.directory);
      setReport(inspected);
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      request.current = false;
      setBusy(false);
    }
  };
  return { directory, report, busy, error, check: () => void run(false, false),
    choose: () => void run(true, false), showUser: () => void run(false, true) };
}

type SourceState = ReturnType<typeof useClientConfigurationSources>;

function sourceLabel(category: "layer" | "state", value: string, t: TFunction): string {
  const key = `clientConfig.sources.${category}.${value}`;
  const translated = catalogText(key, t);
  return translated === key ? value : translated;
}

function Observations({ rows, title }: { rows: ConfigurationObservation[]; title: string }) {
  if (!rows.length) return null;
  return <details className="asb-client-source-values">
    <summary>{title}</summary>
    <FactList rows={rows.map(({ key, value, source }) => ({
      label: <code>{key}</code>,
      value: <><code className="asb-fact-code">{value}</code>{source && <small>{source}</small>}</>,
    }))} />
  </details>;
}

export function ClientConfigurationSourcesDialog({ app, sources, onClose }: {
  app: AppKind; sources: SourceState; onClose: () => void;
}) {
  const { t } = useI18n();
  const { report, busy, directory, error } = sources;
  return <AppDialog title={t("clientConfig.sources.title")} busy={false} wide tall onClose={onClose}>
    <section className="asb-client-configuration-sources" aria-busy={busy}>
      <div className="asb-client-source-actions">
        <Button variant="secondary" onClick={sources.choose} disabled={busy}>{t("clientConfig.sources.choose")}</Button>
        {directory && <Button variant="secondary" onClick={sources.showUser} disabled={busy}>{t("clientConfig.sources.userScope")}</Button>}
        <Button variant="secondary" onClick={sources.check} disabled={busy}>{t("clientConfig.sources.refresh")}</Button>
      </div>
      <p className="asb-field-help"><code>{directory ?? t("clientConfig.sources.userScope")}</code></p>
      <p className="asb-field-help">{t(app === "codex" ? "clientConfig.sources.codexHelp" : "clientConfig.sources.claudeHelp")}</p>
      {busy && <p role="status">{t("clientConfig.sources.checking")}</p>}
      {error && <p role="alert" className="asb-field-error">{commandErrorText(error, t)}</p>}
      {report && <>
        {report.installation && <p className="asb-field-help"><code>{report.installation}</code>{report.version && <> · {report.version}</>}</p>}
        {report.issue && <p role="status" className="asb-field-error">{report.issue}</p>}
        {report.sources.length > 0 && <ul className="asb-client-source-list">
          {report.sources.map(({ label, path, state }, index) => <li key={JSON.stringify([label, path, index])}>
            <span>{sourceLabel("layer", label, t)}</span>
            {path && <code>{path}</code>}
            <span>{sourceLabel("state", state, t)}</span>
          </li>)}
        </ul>}
        <Observations rows={report.values} title={t(app === "codex" ? "clientConfig.sources.merged" : "clientConfig.sources.valuesOnDisk")} />
        <Observations rows={report.requirements} title={t("clientConfig.sources.requirements")} />
        {report.windowsSandbox && <p className="asb-field-help">{t("clientConfig.sources.windowsSandbox", {
          state: catalogText(`clientConfig.sources.sandbox.${report.windowsSandbox}`, t),
        })}</p>}
      </>}
    </section>
  </AppDialog>;
}
