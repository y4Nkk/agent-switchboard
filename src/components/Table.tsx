import type { ReactNode } from "react";

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
      className={className === undefined ? "asb-table" : `asb-table ${className}`}
      aria-label={ariaLabel}
    >
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.key} scope="col">
              {column.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => (
          <tr key={rowKey(row, index)}>
            {columns.map((column) => (
              <td key={column.key} className={column.cellClassName}>
                {column.render(row)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
