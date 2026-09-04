import type { ReactNode } from "react";
import { cx } from "@/utils/cx";

export interface TableColumn<T> {
  /** Stable column identity; also the header/cell React key. */
  key: string;
  header: string;
  /** Renders one row's cell content; cell semantics stay caller-owned. */
  render: (row: T) => ReactNode;
  cellClassName?: string;
}

interface Props<T> {
  columns: Array<TableColumn<T>>;
  rows: T[];
  /** Receives the row index too: some tables hold identical entries. */
  rowKey: (row: T, index: number) => string;
  /** Accessible name of the whole table. */
  ariaLabel: string;
  /** Module-owned width or scroll hints appended to the table element. */
  className?: string;
}

/**
 * The single data-table renderer: one markup and style contract for every
 * tabular module. Scroll ownership stays with each module's own region;
 * visual values come from styles/tokens.css via the .asb-table rules.
 */
export function Table<T>({ columns, rows, rowKey, ariaLabel, className }: Props<T>) {
  return (
    <table
      className={cx("asb-table", className)}
      aria-label={ariaLabel}
    >
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.key} scope="col" className="asb-table-cell">
              {column.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => (
          <tr key={rowKey(row, index)}>
            {columns.map((column) => (
              <td key={column.key} className={cx("asb-table-cell", column.cellClassName)}>
                {column.render(row)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
