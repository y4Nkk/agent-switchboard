import { useEffect, useMemo, useState } from "react";
import type { ObservedExtension } from "../../api/client";
import type { DiscoverScan as ScanState } from "../../app/extensions/useDiscoverScan";
import { discoveryImportMode } from "../../app/extensions/useDiscoveryImport";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Download, RefreshCw } from "lucide-react";
import { DiscoveryWarnings } from "./DiscoveryWarnings";
import { DiscoveryImportList } from "./DiscoveryImportList";
import { ExtensionLoading } from "./ExtensionLoading";
import { useDiscoverySelection } from "./useDiscoverySelection";
import { errorText } from "../../i18n/errors";
import { useI18n } from "../../i18n";
import { diagnosticInView, observationInView, type BindingViewInfo } from "./discovery-view";

type DiscoverScan = ScanState & { repairPreparing: boolean };
export type DiscoverySelection = ReturnType<typeof useDiscoverySelection>;

interface DiscoveryRowDeps {
  discovery: DiscoverScan;
  kindTab: "skill" | "mcp";
  search: string;
  bindingInfo: ReadonlyMap<string, BindingViewInfo>;
}

export function useDiscoveryRows({ discovery, kindTab, search, bindingInfo }: DiscoveryRowDeps) {
  const { snapshot } = discovery;
  return useMemo(() => {
    const filter = { kind: kindTab, client: "all" as const, search };
    const allRows = (snapshot?.observations ?? []).filter((item) => item.kind === kindTab);
    const rows = allRows.filter((item) => observationInView(item, filter));
    const ids = new Set(rows.map((item) => item.observationId));
    const diagnostics = (snapshot?.diagnostics ?? [])
      .filter((item) => diagnosticInView(item, filter, ids, bindingInfo));
    const warnings = new Map<string, number>();
    for (const item of diagnostics) {
      if (item.subject.kind === "discoveryEntry" && item.remediation.kind !== "info") {
        warnings.set(item.subject.observationId, (warnings.get(item.subject.observationId) ?? 0) + 1);
      }
    }
    return { allRows, rows, diagnostics, warnings };
  }, [snapshot, kindTab, search, bindingInfo]);
}

export type DiscoveryRows = ReturnType<typeof useDiscoveryRows>;

function ScanToolbar({ discovery, busy }: { discovery: DiscoverScan; busy: boolean }) {
  const { t } = useI18n();
  return <div className="asb-ext-import-scan">
    <span role="status" className="asb-scope-note">{discovery.scanning ? t("importDiscovery.scan.scanning") : discovery.stale
      ? t("importDiscovery.scan.stale") : discovery.snapshot ? t("importDiscovery.scan.scannedAt", { time: new Date(discovery.snapshot.scannedAt)
        .toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) }) : t("importDiscovery.scan.idle")}</span>
    <Button variant="icon" aria-label={t("importDiscovery.scan.rescan")} title={t("importDiscovery.scan.rescan")} disabled={busy || discovery.scanning}
      onClick={() => void discovery.scan()}><RefreshCw /></Button>
  </div>;
}

/** The fixed bottom bar of the discovery frame: batch selection plus the
 * import commit action, mirroring the provider editor's write bar position. */
export function DiscoveryImportBar({ discovery, busy, selection, rows }: {
  discovery: DiscoverScan;
  busy: boolean;
  selection: DiscoverySelection;
  rows: ObservedExtension[];
}) {
  const { t } = useI18n();
  const selectable = rows.filter((item) => discoveryImportMode(item));
  const all = selectable.length > 0 && selectable.every((item) => selection.selected.has(item.observationId));
  const importing = async () => {
    selection.setResult(null);
    const result = await discovery.importSelected(selection.selectedItems);
    if (result) selection.setResult(result);
  };
  return <div className="asb-ext-import-footer">
    <Checkbox label={t("importDiscovery.importBar.selectAll")} checked={all} disabled={busy || selectable.length === 0}
      onChange={(checked) => selection.selectAll(rows, checked)} />
    <span className="asb-scope-note">{t("importDiscovery.importBar.selected", { count: selection.selectedItems.length })}</span>
    <Button variant="primary" disabled={busy || selection.selectedItems.length === 0}
      onClick={() => void importing()}><Download />{t("importDiscovery.importBar.importSelected", { count: selection.selectedItems.length })}</Button>
  </div>;
}

/** Native imports retain each detected client installation and its recovery baseline. */
export function DiscoverPanel(props: Props) {
  const { discovery, view, selection } = props;
  const { t } = useI18n();
  const { snapshot, ensureInitialScan } = discovery;
  const [open, setOpen] = useState(false);
  const [focusDiagnosticId, setFocusDiagnosticId] = useState<string | null>(null);
  useEffect(() => { ensureInitialScan(); }, [ensureInitialScan]);
  useEffect(() => { setOpen(false); setFocusDiagnosticId(null); }, [discovery.scanId]);
  const surfaceWarning = (id: string) => {
    const first = view.diagnostics.find((item) => item.subject.kind === "discoveryEntry" && item.subject.observationId === id);
    if (first) { setFocusDiagnosticId(first.id); setOpen(true); }
  };
  return <div className="asb-ext-import" aria-label={t("importDiscovery.panel.aria")}>
    <ScanToolbar discovery={discovery} busy={props.busy} />
    {snapshot && <DiscoveryWarnings diagnostics={view.diagnostics} scanId={snapshot.scanId}
      scannedAt={snapshot.scannedAt} stale={discovery.stale} busy={props.busy}
      repairPreparing={discovery.repairPreparing} bindingInfo={props.bindingInfo}
      observationNames={new Map(view.rows.map((item) => [item.observationId, item.name]))}
      focusDiagnosticId={focusDiagnosticId} open={open} onOpenChange={setOpen}
      onRepair={() => props.onRepair(view.diagnostics.filter((item) => item.remediation.kind === "auto").map((item) => item.id))} />}
    {selection.result && selection.result.failed.length > 0 && <ul className="asb-ext-import-errors" role="alert">
      {selection.result.failed.map((item, index) => <li key={index}>{t("importDiscovery.panel.failedItem", { name: item.name, message: errorText(item.error, t) })}</li>)}
    </ul>}
    {view.rows.length === 0 ? (discovery.scanning ? <ExtensionLoading /> : <p className="asb-empty">
      {props.search ? t("importDiscovery.panel.emptyFiltered") : t("importDiscovery.panel.empty")}
    </p>) : <DiscoveryImportList rows={view.rows} selected={selection.selected} busy={props.busy}
      projects={props.projectNames} warnings={view.warnings} onSelect={selection.select}
      onWarning={surfaceWarning} onViewDetails={props.onViewDetails} />}
  </div>;
}

interface Props {
  discovery: DiscoverScan;
  busy: boolean;
  kindTab: "skill" | "mcp";
  search: string;
  projectNames: ReadonlyMap<string, string>;
  bindingInfo: ReadonlyMap<string, BindingViewInfo>;
  /** Row model and selection state owned by the composing discovery view. */
  view: DiscoveryRows;
  selection: DiscoverySelection;
  onViewDetails: (observed: ObservedExtension) => void;
  onRepair: (diagnosticIds: string[]) => void;
}
