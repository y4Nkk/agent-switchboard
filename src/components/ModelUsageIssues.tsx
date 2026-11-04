import { memo, useState } from "react";
import type { ModelUsageIssue } from "../api/client";
import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";
import { CollapsibleSection } from "./Collapsible";

export const ModelUsageIssues = memo(function ModelUsageIssues({ issues }: { issues: ModelUsageIssue[] }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  if (issues.length === 0) return null;
  return <CollapsibleSection open={open} onOpenChange={setOpen} quiet
    label={t("usage.coverage.issues", { count: issues.length })}>
    <ul className="asb-usage-issues">
      {issues.map((issue) => <li key={JSON.stringify([issue.app, issue.message])}>
        {clientName(issue.app)} · {issue.message}
      </li>)}
    </ul>
  </CollapsibleSection>;
});
