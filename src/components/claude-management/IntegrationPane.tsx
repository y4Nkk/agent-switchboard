import { uiMessage } from "../../i18n/errors";
import { useEffect, useRef, useState } from "react";
import * as api from "../../api/claude-integration";
import { useI18n, type MessageKey, type TFunction } from "../../i18n";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { DiffView } from "../DiffView";
import { ClientManagementModule } from "../client-management/ClientManagementModule";
import { useClientToolDialog } from "../client-management/use-client-tool-dialog";
import { MarkerArt, type MarkerState } from "./MarkerArt";
import type { ClaudeOperations } from "./operations";

const FLAGS: Record<api.ClaudeIntegrationFlag, { title: MessageKey; note: MessageKey }> = {
  plugin: { title: "clientConfig.integration.pluginTitle", note: "clientConfig.integration.pluginNote" },
  onboarding: { title: "clientConfig.integration.onboardingTitle", note: "clientConfig.integration.onboardingNote" },
};

function markerState(t: TFunction, flag: api.ClaudeIntegrationFlagState): string {
  return flag.exists ? (flag.applied ? t("clientConfig.integration.written") : t("clientConfig.integration.notWritten")) : t("clientConfig.integration.pendingCreate");
}

function markerArt(flag: api.ClaudeIntegrationFlagState): MarkerState {
  return !flag.exists ? "pending" : flag.applied ? "written" : "absent";
}

function MarkerCard({ flag, busy, pluginIntegration, onPolicyChange, onPreview }: {
  flag: api.ClaudeIntegrationFlagState;
  busy: boolean;
  pluginIntegration: boolean;
  onPolicyChange: (enabled: boolean) => void;
  onPreview: (flag: api.ClaudeIntegrationFlag, enable: boolean, returnFocus: () => HTMLButtonElement | null) => void;
}) {
  const { t } = useI18n();
  const meta = FLAGS[flag.flag];
  const title = t(meta.title);
  const writeButton = useRef<HTMLButtonElement>(null);
  const clearButton = useRef<HTMLButtonElement>(null);
  const returnFocus = () => writeButton.current?.disabled ? clearButton.current : writeButton.current;
  return (
    <section aria-label={title} className="asb-tool-card">
      <div className="asb-marker-head">
        <MarkerArt state={markerArt(flag)} />
        <div className="asb-marker-titles">
          <span className={`asb-status-pill${flag.applied ? " is-ok" : " is-idle"}`}>
            <span className="asb-status-pill-dot" aria-hidden="true" />{markerState(t, flag)}
          </span>
          <h4 className="asb-group-title">{title}</h4>
        </div>
      </div>
      <p className="asb-marker-note">{t(meta.note)}</p>
      <p className="asb-marker-meta">
        <code>{flag.target}</code>{flag.exists ? "" : t("clientConfig.integration.createdOnWrite")}
      </p>
      {flag.flag === "plugin" && <Checkbox label={t("clientConfig.integration.autoSync")}
        checked={pluginIntegration} disabled={busy} onChange={onPolicyChange} />}
      <div className="asb-marker-actions">
        <Button ref={writeButton} variant="secondary" disabled={busy || flag.applied}
          onClick={() => onPreview(flag.flag, true, returnFocus)}>{t("clientConfig.integration.write")}</Button>
        <Button ref={clearButton} variant="secondary" disabled={busy || !flag.applied}
          onClick={() => onPreview(flag.flag, false, returnFocus)}>{t("clientConfig.integration.clear")}</Button>
      </div>
    </section>
  );
}

/** One marker's pending write as a dialog: the change list, then the single
 * confirmation that writes it. A failed write stays here with its reason. */
function MarkerWritePreview({ preview, operations, onClose, onConfirm }: {
  preview: api.ClaudeIntegrationPreview;
  operations: ClaudeOperations;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();
  const title = t(FLAGS[preview.flag].title);
  return <AppDialog title={t("clientConfig.integration.previewTitle", { title })} busy={operations.busy} onClose={onClose} footer={<>
    <Button autoFocus variant="secondary" disabled={operations.busy} onClick={onClose}>{t("clientConfig.common.cancel")}</Button>
    {preview.changes.length > 0 && <Button variant="primary" disabled={operations.busy} onClick={onConfirm}>{t("clientConfig.integration.confirmWrite")}</Button>}
  </>}>
    {operations.error && <p role="alert" className="asb-field-error">{operations.error}</p>}
    <p className="asb-scope-note">{t("clientConfig.integration.previewNote")}</p>
    {preview.changes.length === 0
      ? <p role="status">{t("clientConfig.integration.alreadyApplied")}</p>
      : <DiffView layout="list" label={t("clientConfig.integration.changeLabel", { title })} changes={preview.changes} />}
  </AppDialog>;
}

/** Both markers live in Claude's own client files; every write is previewed,
 * confirmed, locked, backed up, and atomically replaced. Each marker renders
 * as one card — its toggle device, the state, and the write or clear action —
 * and the write preview is the pane's dialog, closed with the pane. */
export function IntegrationPane({ operations: op, active }: { operations: ClaudeOperations; active: boolean }) {
  const { t } = useI18n();
  const [view, setView] = useState<api.ClaudeIntegrationView | null>(null);
  const { run, busy, changed } = op;
  const { dialog: preview, load, close } = useClientToolDialog<api.ClaudeIntegrationPreview>(active, busy);
  const reload = () => void run(async () => setView(await api.getClaudeIntegration()));
  const changePolicy = (pluginIntegration: boolean) => void run(async () => {
    setView(await api.setClaudeIntegrationPolicy({ pluginIntegration }, true));
    changed(uiMessage("clientConfig.integration.policySaved"));
  });
  useEffect(() => { void run(async () => setView(await api.getClaudeIntegration())); }, [run]);

  return (
    <ClientManagementModule
      title={t("clientConfig.integration.title")}
      refreshLabel={t("clientConfig.integration.refreshLabel")}
      busy={busy}
      onRefresh={reload}
    >
      {view ? <div className="asb-marker-grid">{view.flags.map((flag) => (
        <MarkerCard key={flag.flag} flag={flag} busy={busy} pluginIntegration={view.policy.pluginIntegration}
          onPolicyChange={changePolicy}
          onPreview={(name, enable, returnFocus) => void load(() => api.previewClaudeIntegration(name, enable), run, returnFocus)} />
      ))}</div> : busy && <div className="asb-marker-grid" role="status" aria-label={t("clientConfig.loading")}>
        <div className="asb-marker-skeleton asb-skeleton" aria-hidden="true" />
        <div className="asb-marker-skeleton asb-skeleton" aria-hidden="true" />
      </div>}
      {preview && <MarkerWritePreview preview={preview} operations={op} onClose={close}
        onConfirm={() => void run(async () => {
          setView(await api.applyClaudeIntegration(preview, true));
          close();
          changed(uiMessage("clientConfig.integration.appliedNotice"));
        })} />}
    </ClientManagementModule>
  );
}
