import type { ReactNode } from "react";

export interface FactListRow {
  label: ReactNode;
  value: ReactNode;
}

/** The one key/value facts primitive: every pair renders as
 * <div><dt/><dd/></div> inside a single two-column grid, so all labels share
 * the widest label's column and all values start at the same left edge.
 * `inline` keeps pair wrappers visible for horizontal groups.
 * DOM and visual contract are owned by `.asb-fact-row` in
 * styles/base/panels.css; value-internal layout stays in the row value. */
export function FactList({ rows, inline = false }: { rows: FactListRow[]; inline?: boolean }) {
  return (
    <dl className={inline ? "asb-fact-row is-inline" : "asb-fact-row"}>
      {rows.map((row, index) => (
        <div key={index}>
          <dt>{row.label}</dt>
          <dd>{row.value}</dd>
        </div>
      ))}
    </dl>
  );
}
