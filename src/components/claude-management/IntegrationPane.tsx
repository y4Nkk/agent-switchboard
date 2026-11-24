import { uiMessage } from "../../i18n/errors";
import { useEffect, useState } from "react";
import * as api from "../../api/claude-integration";
import { useI18n, type MessageKey, type TFunction } from "../../i18n";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { DiffView } from "../DiffView";
import { ClientManagementModule } from "../client-management/ClientManagementModule";
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

/** Both markers live in Claude's own client files; every write is previewed,
 * confirmed, locked, backed up, and atomically replaced. Each marker renders
 * as one card — its toggle device, the state, and the write or clear action —
 * whose preview opens inside the card. */
export function IntegrationPane({ operations: op }: { operations: ClaudeOperations }) {
  const { t } = useI18n();
  const [view, setView] = useState<api.ClaudeIntegrationView | null>(null);
  const [preview, setPreview] = useState<api.ClaudeIntegrationPreview | null>(null);
  const { run, busy, changed } = op;
  const reload = () => void run(async () => setView(await api.getClaudeIntegration()));
  const plan = (flag: api.ClaudeIntegrationFlag, enable: boolean) => void run(async () => {
    setPreview(await api.previewClaudeIntegration(flag, enable));
  });

  useEffect(() => { void run(async () => setView(await api.getClaudeIntegration())); }, [run]);

  return (
    <ClientManagementModule
      title={t("clientConfig.integration.title")}
      refreshLabel={t("clientConfig.integration.refreshLabel")}
      busy={busy}
      refreshDisabled={busy || !!preview}
      onRefresh={reload}
    >
      {view ? <div className="asb-marker-grid">{view.flags.map((flag) => {
        const meta = FLAGS[flag.flag];
        const rowPreview = preview?.flag === flag.flag ? preview : null;
        const title = t(meta.title);
        return (
          <section key={flag.flag} aria-label={title}
            className={"asb-tool-card asb-marker-card" + (rowPreview ? " is-previewing" : "")}>
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
            {flag.flag === "plugin" && (
              <Checkbox
                label={t("clientConfig.integration.autoSync")}
                checked={view.policy.pluginIntegration}
                disabled={busy || !!preview}
                onChange={(pluginIntegration) => void run(async () => {
                  setView(await api.setClaudeIntegrationPolicy({ pluginIntegration }, true));
                  changed(uiMessage("clientConfig.integration.policySaved"));
                })}
              />
            )}
            <div className="asb-marker-actions">
              <Button variant="secondary" disabled={busy || !!preview || flag.applied} onClick={() => plan(flag.flag, true)}>{t("clientConfig.integration.write")}</Button>
              <Button variant="secondary" disabled={busy || !!preview || !flag.applied} onClick={() => plan(flag.flag, false)}>{t("clientConfig.integration.clear")}</Button>
            </div>
            {rowPreview && (
              <section aria-label={t("clientConfig.integration.previewAria", { title })} className="asb-marker-preview">
                <p className="asb-scope-note">{t("clientConfig.integration.previewNote")}</p>
                {rowPreview.changes.length === 0
                  ? <p role="status">{t("clientConfig.integration.alreadyApplied")}</p>
                  : <DiffView layout="list" label={t("clientConfig.integration.changeLabel", { title })} changes={rowPreview.changes} />}
                <div className="asb-form-actions">
                  <Button variant="secondary" disabled={busy} onClick={() => setPreview(null)}>{t("clientConfig.common.cancel")}</Button>
                  {rowPreview.changes.length > 0 && <Button variant="primary" disabled={busy} onClick={() => void run(async () => {
                    setView(await api.applyClaudeIntegration(rowPreview, true));
                    setPreview(null);
                    changed(uiMessage("clientConfig.integration.appliedNotice"));
                  })}>{t("clientConfig.integration.confirmWrite")}</Button>}
                </div>
              </section>
            )}
          </section>
        );
      })}</div> : <div className="asb-marker-grid" role="status" aria-label={t("clientConfig.loading")}>
        <div className="asb-marker-skeleton asb-skeleton" aria-hidden="true" />
        <div className="asb-marker-skeleton asb-skeleton" aria-hidden="true" />
      </div>}
    </ClientManagementModule>
  );
}
