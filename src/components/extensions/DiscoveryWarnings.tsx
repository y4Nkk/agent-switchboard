import { useEffect, useId, useRef } from "react";
import type { ExtensionDiagnostic } from "../../api/client";
import { Button } from "../Button";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { clientName } from "../../lib/client-name";
import { useI18n, type TFunction } from "../../i18n";
import { catalogText } from "../../i18n/errors";
import { DIAGNOSTIC_CODE_LABELS, REMEDIATION_LABELS } from "./labels";
import type { BindingViewInfo } from "./discovery-view";

interface Props {
  diagnostics: ExtensionDiagnostic[];
  /** Identity of the scan the diagnostics belong to; collapsing resets on
   * every new scan. */
  scanId: string;
  scannedAt: string | null;
  /** True when the last scan attempt failed and these facts were not
   * refreshed. */
  stale: boolean;
  busy: boolean;
  repairPreparing: boolean;
  /** observationId → display name, for entry-subject diagnostics. */
  observationNames: ReadonlyMap<string, string>;
  /** bindingId → library facts, for managed-binding diagnostics. */
  bindingInfo: ReadonlyMap<string, BindingViewInfo>;
  /** A diagnostic id the table wants surfaced; it is highlighted while set. */
  focusDiagnosticId: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRepair: () => void;
}

function subjectLabel(
  diagnostic: ExtensionDiagnostic,
  observationNames: ReadonlyMap<string, string>,
  bindingInfo: ReadonlyMap<string, BindingViewInfo>,
  t: TFunction,
): string {
  switch (diagnostic.subject.kind) {
    case "discoveryEntry":
      return observationNames.get(diagnostic.subject.observationId) ?? t("importDiscovery.warn.subjectEntry");
    case "managedBinding":
      return bindingInfo.get(diagnostic.subject.bindingId)?.name ?? t("importDiscovery.warn.subjectManaged");
    case "scanLocation":
      return diagnostic.subject.label;
  }
}

function remediationText(diagnostic: ExtensionDiagnostic, t: TFunction): string {
  switch (diagnostic.remediation.kind) {
    case "auto":
      return t("importDiscovery.warn.remediationLine", { label: t(REMEDIATION_LABELS.auto), reason: diagnostic.remediation.reason });
    case "manual":
      return t("importDiscovery.warn.remediationLine", { label: t(REMEDIATION_LABELS.manual), reason: diagnostic.remediation.reason });
    case "info":
      return t(REMEDIATION_LABELS.info);
  }
}

function DiagnosticList({ diagnostics, observationNames, bindingInfo, focusDiagnosticId }: Pick<Props,
  "diagnostics" | "observationNames" | "bindingInfo" | "focusDiagnosticId">) {
  const { t } = useI18n();
  return <ul className="asb-discovery-diagnostic-list">
    {diagnostics.map((diagnostic) => (
      <li key={diagnostic.id} className={focusDiagnosticId === diagnostic.id
        ? "asb-discovery-diagnostic is-focused" : "asb-discovery-diagnostic"}>
        <p className="asb-discovery-diagnostic-head">
          <span className="asb-pill-status">
            {catalogText(DIAGNOSTIC_CODE_LABELS[diagnostic.code] ?? diagnostic.code, t)}
          </span>
          <strong>{subjectLabel(diagnostic, observationNames, bindingInfo, t)}</strong>
          <span className="asb-scope-note">{clientName(diagnostic.client)}</span>
        </p>
        <p>{diagnostic.message}</p>
        <p className="asb-scope-note">{remediationText(diagnostic, t)}</p>
      </li>
    ))}
  </ul>;
}

/** The single collapsible surface for discovery diagnostics. Collapsed it is
 * exactly one line — the counted summary row is itself the only expand and
 * collapse control, with the repair action beside it; expanded it lists every
 * diagnostic with its own identity, reason, and remediation class. */
export function DiscoveryWarnings(props: Props) {
  const { diagnostics, scanId, scannedAt, stale, busy, repairPreparing, open, onOpenChange, onRepair } = props;
  const { t } = useI18n();
  const detailsId = useId();
  const lastScanIdRef = useRef(scanId);

  // Every new scan starts collapsed again: the counted summary is the
  // resting state, never a persisted setting. The very first mount keeps
  // whatever the parent chose.
  useEffect(() => {
    if (lastScanIdRef.current === scanId) return;
    lastScanIdRef.current = scanId;
    onOpenChange(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scanId]);

  const warnings = diagnostics.filter(
    (diagnostic) => diagnostic.remediation.kind !== "info",
  );
  const informationCount = diagnostics.length - warnings.length;
  if (warnings.length === 0 && informationCount === 0) return null;

  const repairable = warnings.filter(
    (diagnostic) => diagnostic.remediation.kind === "auto",
  ).length;
  const summary = t("importDiscovery.warn.summary", { warnings: warnings.length, repairable }) +
    (informationCount > 0 ? t("importDiscovery.warn.summaryInfo", { count: informationCount }) : "");

  return (
    <section className="asb-discovery-warnings" aria-label={t("importDiscovery.warn.aria")}>
      <div className="asb-banner asb-banner-warning asb-discovery-summary">
        <CollapsibleTrigger open={open} onOpenChange={onOpenChange} contentId={detailsId}
          className="asb-discovery-summary-toggle"
          label={summary + (stale ? t("importDiscovery.warn.staleSuffix") : "")} />
        <div className="asb-discovery-summary-actions">
          {repairable > 0 ? (
            <Button
              variant="primary"
              disabled={busy || repairPreparing}
              aria-label={t("importDiscovery.warn.repairAria", { count: repairable })}
              onClick={onRepair}
            >
              {repairPreparing ? t("importDiscovery.warn.preparing") : t("importDiscovery.warn.repair", { count: repairable })}
            </Button>
          ) : warnings.length > 0 ? (
            <span className="asb-scope-note">{t("importDiscovery.warn.noAuto")}</span>
          ) : null}
        </div>
      </div>
      {stale && scannedAt && (
        <p className="asb-scope-note" role="status">
          {t("importDiscovery.warn.staleNote", { time: scannedAt })}
        </p>
      )}
      <Collapsible open={open} id={detailsId}>
        <DiagnosticList diagnostics={diagnostics} observationNames={props.observationNames}
          bindingInfo={props.bindingInfo} focusDiagnosticId={props.focusDiagnosticId} />
      </Collapsible>
    </section>
  );
}
