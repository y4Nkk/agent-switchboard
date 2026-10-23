import { useId } from "react";
import type { ResourceGroup, ResourceGroupReading } from "../api/client";
import { Button } from "../components/Button";
import { Collapsible, CollapsibleChevron } from "../components/Collapsible";
import { Table, type TableColumn } from "../components/Table";
import { ResourceMetric, resourceIssueText } from "../components/resource-usage/ResourceMetric";
import { ResourceProcessDetails } from "../components/resource-usage/ResourceProcessDetails";
import { useI18n, type TFunction } from "../i18n";
import type { useResourceUsage } from "./use-resource-usage";

type Resources = ReturnType<typeof useResourceUsage>;

function groupName(group: ResourceGroup, t: TFunction) { return t(`resources.group.${group}`); }
const INTERFACE_ISSUES = new Set(["webviewPartial", "webviewUnavailable", "browserExcluded"]);

function columns(resources: Resources, id: string, t: TFunction): TableColumn<ResourceGroupReading>[] {
  return [
    { key: "software", header: t("resources.column.software"), render: (row) => {
      const open = resources.selection?.group === row.group;
      return <div className="asb-resource-software">
        <Button variant="unstyled" size="compact" className="asb-resource-name"
          aria-expanded={open} aria-controls={open ? `${id}-${row.group}` : undefined}
          onClick={() => resources.toggle(row.group)}>
          <CollapsibleChevron open={open} />{groupName(row.group, t)}
        </Button>
        {row.issues.length > 0 && <p className="asb-resource-note">
          {t("resources.identified")}
          {row.issues.filter((issue) => !INTERFACE_ISSUES.has(issue))
            .map((issue) => <span key={issue}>{" · "}{resourceIssueText(issue, t)}</span>)}
        </p>}
      </div>;
    } },
    { key: "presence", header: t("resources.column.status"), render: (row) => t(`resources.presence.${row.presence}`) },
    { key: "cpu", header: "CPU", align: "end", render: (row) => <ResourceMetric metric={row.cpu} kind="cpu" /> },
    { key: "memory", header: t("resources.column.memory"), align: "end",
      render: (row) => <ResourceMetric metric={row.residentBytes} kind="memory" /> },
    { key: "processes", header: t("resources.column.count"), align: "end",
      render: (row) => <ResourceMetric metric={row.processCount} kind="count" /> },
  ];
}

export function ResourceUsagePage({ resources }: { resources: Resources }) {
  const { t } = useI18n();
  const id = useId();
  return <div className="asb-resource-usage" aria-busy={resources.loading}>
    <p className="asb-field-help">{t("resources.scope")}</p>
    {resources.read && <p className="asb-field-help">
      {t("resources.platform", { name: ({ windows: "Windows", macos: "macOS", linux: "Linux" } as Record<string, string>)[resources.read.scope.platform] ?? resources.read.scope.platform })}
      {resources.read.scope.interfaceCoverage === "browserExcluded" && <> · {t("resources.issue.browserExcluded")}</>}
      {resources.read.scope.interfaceCoverage === "partial" && <> · {t("resources.issue.webviewPartial")}</>}
    </p>}
    {resources.error && <p className="asb-warn-text" role="alert">
      {resources.error}{resources.read && <> {t("resources.previousReading")}</>}
    </p>}
    {resources.read ? <Table columns={columns(resources, id, t)} rows={resources.read.groups}
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
    <p className="asb-field-help">{t("resources.memoryNote")}</p>
    {resources.read && resources.read.scope.issues.length > 0 && <p className="asb-field-help">
      {resources.read.scope.issues.map((issue) => resourceIssueText(issue, t)).join(" · ")}
    </p>}
  </div>;
}
