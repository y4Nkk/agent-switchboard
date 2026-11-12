import { useCallback, useEffect, useRef, useState } from "react";
import { getOutboundProxySettings, setOutboundProxySettings,
  type OutboundProxyMode, type OutboundProxyView as View } from "../api/outbound-proxy";
import type { CommandError } from "../api/shared";
import { useI18n } from "../i18n";
import { useMessageState } from "../i18n/use-message-state";
import { Button } from "./Button";
import { Input } from "./Input";
import { AppSettingRow } from "./AppPreferenceControls";
import { RadioOption } from "./RadioOption";

const MODES: readonly OutboundProxyMode[] = ["none", "system", "manual"];

interface Props {
  active: boolean;
  busy: boolean;
  revision: number;
  onBusy: (busy: boolean) => void;
  onError: (error: CommandError) => void;
  onRecovered: () => void;
}

function useProxyEditor({ active, busy, revision, onBusy, onError, onRecovered }: Props) {
  const [view, setView] = useState<View | null>(null);
  const acceptedRevision = useRef<string | null>(null);
  const [mode, setMode] = useState<OutboundProxyMode>("none");
  const [url, setUrl] = useState("");
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  const generation = useRef(0);
  const locked = useRef(false);
  useEffect(() => () => { generation.current += 1; }, []);
  const accept = useCallback((next: View) => {
    setView(next); setError(next.issue);
    if (acceptedRevision.current !== next.revision) {
      acceptedRevision.current = next.revision;
      setMode(next.settings?.mode ?? "manual"); setUrl(next.settings?.url ?? "");
    }
    if (!next.issue) onRecovered();
  }, [onRecovered, setError]);
  const refresh = useCallback(async () => {
    if (locked.current) return;
    const request = ++generation.current;
    setLoading(true); setError(null); setSaved(false);
    try {
      const next = await getOutboundProxySettings();
      if (request === generation.current) accept(next);
    } catch (caught) {
      if (request !== generation.current) return;
      setView(null); setError(caught); onError(caught as CommandError);
    } finally { if (request === generation.current) setLoading(false); }
  }, [accept, onError, setError]);
  useEffect(() => { if (active) void refresh(); }, [active, revision, refresh]);
  const save = async () => {
    if (busy || locked.current || !view) return;
    locked.current = true;
    const request = ++generation.current;
    setLoading(false); setWorking(true); setError(null); setSaved(false); onBusy(true);
    try {
      const next = await setOutboundProxySettings({ mode, url: mode === "manual" ? url.trim() : null }, view.revision);
      if (request !== generation.current) return;
      accept(next); setSaved(true);
    } catch (caught) { if (request === generation.current) setError(caught); }
    finally {
      locked.current = false; onBusy(false);
      if (request === generation.current) setWorking(false);
    }
  };
  return { view, mode, setMode, url, setUrl, saved, setSaved, loading, working, error, refresh, save };
}

/** Explicit connection editing remains available when stored policy is invalid. */
export function OutboundProxySettings(props: Props) {
  const { t } = useI18n();
  const state = useProxyEditor(props);
  const disabled = props.busy || state.working || state.loading || state.view === null;
  const dirty = state.view !== null && (!state.view.settings || state.view.settings.mode !== state.mode
    || (state.mode === "manual" && state.view.settings.url !== state.url.trim()));
  return <section className="asb-app-settings-group" aria-labelledby="egress-settings" aria-busy={state.loading || state.working}>
    <h3 id="egress-settings" className="asb-section-title">{t("settings.egress.title")}</h3>
    <p className="asb-field-help">{t("settings.egress.description")}</p>
    <AppSettingRow label={t("settings.egress.mode")}>
      <div className="asb-segments" role="radiogroup" aria-label={t("settings.egress.mode")}>
        {MODES.map((value) => <RadioOption key={value} name="egress-mode"
          label={t(`settings.egress.mode.${value}`)} checked={state.mode === value} disabled={disabled}
          onChange={() => { state.setMode(value); state.setSaved(false); }} />)}
      </div>
    </AppSettingRow>
    {state.mode === "manual" && <AppSettingRow label={t("settings.egress.url")} detail={t("settings.egress.urlHelp")}>
      <Input type="text" value={state.url} disabled={disabled} spellCheck={false}
        onChange={(event) => { state.setUrl(event.target.value); state.setSaved(false); }} />
    </AppSettingRow>}
    {state.mode === "system" && <p className="asb-field-help">{t("settings.egress.systemNote")}</p>}
    {state.error && <p className="asb-field-error" role="alert">{state.error}</p>}
    <div className="asb-session-actions">
      <Button variant="secondary" disabled={props.busy || state.working || state.loading}
        onClick={() => void state.refresh()}>{t("settings.egress.reload")}</Button>
      <Button variant="secondary" disabled={disabled || (!dirty && state.mode !== "system")}
        onClick={() => void state.save()}>{t("settings.egress.save")}</Button>
      {state.saved && <span className="asb-ok-text">{t("settings.egress.saved")}</span>}
    </div>
  </section>;
}
