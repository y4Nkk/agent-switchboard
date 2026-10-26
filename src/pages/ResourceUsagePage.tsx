import { useId } from "react";
import type { ResourceGroup, ResourceGroupReading } from "../api/client";
import { Collapsible, CollapsibleTrigger } from "../components/Collapsible";
import { Table, type TableColumn } from "../components/Table";
import { ResourceMetric, formatResourceValue, resourceIssueText } from "../components/resource-usage/ResourceMetric";
import { ResourceProcessDetails } from "../components/resource-usage/ResourceProcessDetails";
import { ResourceUsageRing } from "../components/resource-usage/ResourceUsageRing";
import { useI18n, type TFunction } from "../i18n";
import type { useResourceUsage } from "./use-resource-usage";

type Resources = ReturnType<typeof useResourceUsage>;

function groupName(group: ResourceGroup, t: TFunction) { return t(`resources.group.${group}`); }
const INTERFACE_ISSUES = new Set(["webviewPartial", "webviewUnavailable", "browserExcluded"]);

function columns(resources: Resources, id: string, t: TFunction, language: string): TableColumn<ResourceGroupReading>[] {
  const memory = (resources.read?.groups ?? []).flatMap((row) => row.residentBytes.value === null ? [] : [row.residentBytes.value]);
  const memoryMaximum = memory.length ? 2 ** Math.ceil(Math.log2(Math.max(1024 ** 2, ...memory))) : null;
  return [
    { key: "software", header: t("resources.column.software"), render: (row) => {
      const open = resources.selection?.group === row.group;
      return <div className="asb-resource-software" data-resource-group={row.group}>
        <CollapsibleTrigger open={open} contentId={`${id}-${row.group}`}
          label={<span className="asb-resource-name-label">
            <span className="asb-resource-series-dot" aria-hidden="true" />{groupName(row.group, t)}
          </span>} onOpenChange={() => resources.toggle(row.group)} />
        <p className="asb-resource-note">{t(`resources.presence.${row.presence}`)}</p>
        {row.issues.length > 0 && <p className="asb-resource-note">
          {t("resources.identified")}
          {row.issues.filter((issue) => !INTERFACE_ISSUES.has(issue))
            .map((issue) => <span key={issue}>{" · "}{resourceIssueText(issue, t)}</span>)}
        </p>}
      </div>;
    } },
    { key: "cpu", header: t("resources.chart.cpu"), cellClassName: "asb-resource-chart-cpu",
      render: (row) => <ResourceUsageRing group={row.group} metric={row.cpu} kind="cpu" maximum={100} /> },
    { key: "memory", header: memoryMaximum === null ? t("resources.column.memory")
      : t("resources.chart.memory", { maximum: formatResourceValue(memoryMaximum, "memory", language) }),
      cellClassName: "asb-resource-chart-memory",
      render: (row) => <ResourceUsageRing group={row.group} metric={row.residentBytes} kind="memory" maximum={memoryMaximum} /> },
    { key: "processes", header: t("resources.column.count"),
      render: (row) => <ResourceMetric metric={row.processCount} kind="count" /> },
  ];
}

export function ResourceUsagePage({ resources }: { resources: Resources }) {
  const { t, language } = useI18n();
  const id = useId();
  return <div className="asb-resource-usage" aria-busy={resources.loading}>
    {resources.read && <p className="asb-field-help">
      {t("resources.platform", { name: ({ windows: "Windows", macos: "macOS", linux: "Linux" } as Record<string, string>)[resources.read.scope.platform] ?? resources.read.scope.platform })}
      {resources.read.scope.interfaceCoverage === "browserExcluded" && <> · {t("resources.issue.browserExcluded")}</>}
      {resources.read.scope.interfaceCoverage === "partial" && <> · {t("resources.issue.webviewPartial")}</>}
    </p>}
    {resources.error && <p className="asb-warn-text" role="alert">
      {resources.error}{resources.read && <> {t("resources.previousReading")}</>}
    </p>}
    {resources.read ? <Table columns={columns(resources, id, t, language)} rows={resources.read.groups}
      rowKey={(row) => row.group} ariaLabel={t("resources.table.aria")} className="asb-resource-table"
      renderExpansion={(row) => <Collapsible open={resources.selection?.group === row.group}
        id={`${id}-${row.group}`} ariaLabel={t("resources.details.for", { name: groupName(row.group, t) })}>
        <ResourceProcessDetails details={resources.details?.group === row.group ? resources.details : null}
          loading={resources.loading} onPageChange={resources.setPage} />
      </Collapsible>} />
      : <div className="asb-resource-loading" role="status">
        <p className="asb-field-help">{t(resources.loading ? "resources.loading" : "resources.noReadings")}</p>
        {resources.loading && <div className="asb-skeleton" />}
      </div>}
    <div className="asb-resource-footnotes">
      <p className="asb-field-help">{t("resources.chart.memoryNote")}</p>
      <p className="asb-field-help">{t("resources.memoryNote")}</p>
      <p className="asb-field-help">{t("resources.scope")}</p>
      {resources.read && resources.read.scope.issues.length > 0 && <p className="asb-field-help">
        {resources.read.scope.issues.map((issue) => resourceIssueText(issue, t)).join(" · ")}
      </p>}
    </div>
  </div>;
}
