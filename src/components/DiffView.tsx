import type { KeyChange } from "../api/client";
import { useI18n } from "../i18n";
import { Table, type TableColumn } from "./Table";

interface Props {
  changes: KeyChange[];
  label: string;
  layout?: "table" | "list";
}

export function DiffView({ changes, label, layout = "table" }: Props) {
  const { t } = useI18n();
  if (changes.length === 0) return null;
  const beforeLabel = t("providers.diff.column.before");
  const afterLabel = t("providers.diff.column.after");
  const before = (change: KeyChange) => change.before === null
    ? <span className="asb-diff-none">{t("providers.diff.none")}</span>
    : <span className="asb-diff-old">{change.before}</span>;
  const after = (change: KeyChange) => change.after === null
    ? <span className="asb-diff-remove">{t("providers.diff.removed")}</span>
    : <span className="asb-diff-new">{change.after}</span>;
  if (layout === "list") {
    return <ul className="asb-diff-list" role="list" aria-label={label}>
      {changes.map((change) => <li key={change.key}>
        <code className="asb-diff-key">{change.key}</code>
        <dl>
          <div><dt>{beforeLabel}</dt><dd>{before(change)}</dd></div>
          <div><dt>{afterLabel}</dt><dd>{after(change)}</dd></div>
        </dl>
      </li>)}
    </ul>;
  }
  const columns: Array<TableColumn<KeyChange>> = [
    { key: "key", header: t("providers.diff.column.key"), cellClassName: "asb-diff-key", render: (change) => change.key },
    { key: "before", header: beforeLabel, cellClassName: "asb-diff-value", render: before },
    { key: "after", header: afterLabel, cellClassName: "asb-diff-value", render: after },
  ];
  return (
    <Table columns={columns} rows={changes} rowKey={(change) => change.key}
      ariaLabel={label} className="asb-diff" />
  );
}
