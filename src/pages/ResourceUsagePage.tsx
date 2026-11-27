import { useEffect, useId, useRef, useState } from "react";
import type { ResourceGroup, ResourceGroupReading, ResourceIssue } from "../api/client";
import { Button } from "../components/Button";
import { Collapsible, CollapsibleSection, CollapsibleTrigger } from "../components/Collapsible";
import { FactStatus } from "../components/FactList";
import { Time } from "../components/Time";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { ResourceMetric, formatResourceValue, resourceIssueText } from "../components/resource-usage/ResourceMetric";
import { ResourceProcessDetails } from "../components/resource-usage/ResourceProcessDetails";
import { ResourceUsageRing } from "../components/resource-usage/ResourceUsageRing";
import { useI18n } from "../i18n";
import type { useResourceUsage } from "./use-resource-usage";

type Resources = ReturnType<typeof useResourceUsage>;
const GROUPS: readonly ResourceGroup[] = ["switchboard", "codex", "claude"];
const PLATFORM_NAMES: Record<string, string> = { windows: "Windows", macos: "macOS", linux: "Linux" };

function ResourceCard({ row, maximum, scopeIssues, id, open, onToggle }: {
  row: ResourceGroupReading; maximum: number | null; scopeIssues: readonly ResourceIssue[];
  id: string; open: boolean; onToggle: () => void;
}) {
  const { t } = useI18n();
  const issues = row.issues.filter((issue) => !scopeIssues.includes(issue));
  const name = t(`resources.group.${row.group}`);
  return <article className="asb-resource-card asb-usage-surface" aria-labelledby={`${id}-name`} data-resource-group={row.group}>
    <div className="asb-resource-card-head">
      <h4 className="asb-resource-name-label asb-group-title" id={`${id}-name`}>
        <span className="asb-resource-series-dot" aria-hidden="true" />{name}
      </h4>
      <FactStatus tone={row.presence === "running" ? "success" : row.presence === "indeterminate" ? "warning" : "neutral"}>
        {t(`resources.presence.${row.presence}`)}
      </FactStatus>
    </div>
    <dl className="asb-resource-card-readings">
      <div><dt>CPU</dt><dd><ResourceUsageRing group={row.group} metric={row.cpu} kind="cpu" maximum={100} /></dd></div>
      <div><dt>{t("resources.column.memory")}</dt><dd>
        <ResourceUsageRing group={row.group} metric={row.residentBytes} kind="memory" maximum={maximum} />
      </dd></div>
    </dl>
    {issues.length > 0 && <p className="asb-resource-note">{issues.map((issue) => resourceIssueText(issue, t)).join(" · ")}</p>}
    <div className="asb-resource-card-footer">
      <dl><dt>{t("resources.column.count")}</dt><dd><ResourceMetric metric={row.processCount} kind="count" /></dd></dl>
      <CollapsibleTrigger open={open} contentId={id} label={t("resources.details.title")} onOpenChange={onToggle} />
    </div>
  </article>;
}

function ResourceNotes({ platform }: { platform: string }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <CollapsibleSection open={open} onOpenChange={setOpen} quiet label={t("resources.measurementNotes")}
    value={PLATFORM_NAMES[platform] ?? platform}>
    <div className="asb-resource-footnotes">
      <p className="asb-field-help">{t(platform === "macos" || platform === "linux" ? "resources.memoryNoteSharedPages" : "resources.memoryNote")}</p>
      <p className="asb-field-help">{t("resources.chart.memoryNote")}</p>
      <p className="asb-field-help">{t("resources.scope")}</p>
    </div>
  </CollapsibleSection>;
}

function samplingIssues(read: NonNullable<Resources["read"]>): ResourceIssue[] {
  const issues = new Set(read.scope.issues);
  if (read.scope.interfaceCoverage === "partial") issues.add("webviewPartial");
  if (read.scope.interfaceCoverage === "browserExcluded") issues.add("browserExcluded");
  return [...issues];
}

function ResourceGroups({ resources, id }: { resources: Resources & { read: NonNullable<Resources["read"]> }; id: string }) {
  const { t } = useI18n();
  const { read, selection } = resources;
  const requestedScroll = useRef<ResourceGroup | null>(null);
  const group = selection?.group;
  useEffect(() => {
    if (requestedScroll.current !== group) return;
    requestedScroll.current = null;
    const frame = requestAnimationFrame(() => document.getElementById(`${id}-${group}`)?.scrollIntoView({ block: "nearest" }));
    return () => cancelAnimationFrame(frame);
  }, [group, id]);
  const issues = samplingIssues(read);
  return <>
    {issues.length > 0 && <p className="asb-resource-note">{issues.map((issue) => resourceIssueText(issue, t)).join(" · ")}</p>}
    <div className="asb-resource-card-grid">
      {read.groups.map((row) => <ResourceCard key={row.group} row={row} maximum={resources.memoryMaximum} scopeIssues={issues}
        id={`${id}-${row.group}`} open={group === row.group} onToggle={() => {
          requestedScroll.current = group === row.group ? null : row.group;
          resources.toggle(row.group);
        }} />)}
    </div>
    {read.groups.map((row) => {
      const label = t("resources.details.for", { name: t(`resources.group.${row.group}`) });
      const detailId = `${id}-${row.group}`;
      return <Collapsible key={row.group} open={group === row.group} id={detailId} ariaLabel={label}>
        <div className="asb-usage-surface">
          <ModuleHeader title={label} />
          <ResourceProcessDetails details={resources.details?.group === row.group ? resources.details : null}
            requestedPage={selection?.group === row.group ? selection.page : null}
            loading={resources.detailsLoading} onPageChange={resources.setPage} />
        </div>
      </Collapsible>;
    })}
    <ResourceNotes platform={read.scope.platform} />
  </>;
}

export function ResourceUsagePage({ resources, active }: { resources: Resources; active: boolean }) {
  const { t, language } = useI18n();
  const id = useId();
  const { read, memoryMaximum, refreshing, initialLoading, resampling } = resources;
  return <section className="asb-usage-stack asb-resource-usage" aria-labelledby={`${id}-heading`} aria-busy={initialLoading}>
    <ModuleHeader id={`${id}-heading`} title={t("resources.overview")} primary={read && <div className="asb-resource-meta">
      <span>{t(resampling ? "resources.resampling" : "resources.sampledAt")}<Time iso={read.sampledAt} precision="second" /></span>
      {memoryMaximum !== null && <span>{t("resources.chart.memory", { maximum: formatResourceValue(memoryMaximum, "memory", language) })}</span>}
    </div>} primaryActions={<Button variant="secondary" className="asb-resource-refresh" aria-busy={refreshing}
      disabled={initialLoading || refreshing || !active} onClick={resources.refresh}>
      {t(refreshing ? "usage.action.refreshing" : "usage.action.refresh")}
    </Button>} />
    {resources.error && <p className="asb-warn-text asb-resource-error" role="alert">
      {resources.error}{read && <> {t("resources.previousReading")}</>}
    </p>}
    {read ? <ResourceGroups resources={{ ...resources, read }} id={id} /> : initialLoading
      ? <div className="asb-resource-card-grid" role="status" aria-label={t("resources.loading")}>
        {GROUPS.map((item) => <span key={item} className="asb-skeleton asb-resource-card-skeleton" aria-hidden="true" />)}
      </div>
      : !resources.error && <p className="asb-field-help">{t("resources.noReadings")}</p>}
  </section>;
}
