import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Table, type TableColumn } from "./Table";

interface Row {
  id: string;
  name: string;
  size: number;
}

const columns: Array<TableColumn<Row>> = [
  { key: "name", header: "名称", render: (row) => row.name },
  {
    key: "size",
    header: "大小",
    cellClassName: "asb-code",
    render: (row) => `${row.size} MB`,
  },
];

const rows: Row[] = [
  { id: "a", name: "中继 A", size: 1 },
  { id: "b", name: "中继 B", size: 2 },
];

function renderTable(props: Partial<Parameters<typeof Table<Row>>[0]> = {}) {
  return render(
    <Table
      columns={columns}
      rows={rows}
      rowKey={(row) => row.id}
      ariaLabel="示例表格"
      {...props}
    />,
  );
}

describe("Table", () => {
  it("renders headers and one body row per entry", () => {
    renderTable();

    expect(screen.getByRole("table", { name: "示例表格" })).toBeInTheDocument();
    expect(screen.getAllByRole("columnheader").map((node) => node.textContent)).toEqual([
      "名称",
      "大小",
    ]);
    expect(screen.getAllByRole("row")).toHaveLength(3);
    expect(screen.getByText("中继 B")).toBeInTheDocument();
  });

  it("renders cell content through the column contract with its cell class", () => {
    renderTable();

    const sizeCell = screen.getByText("2 MB");
    expect(sizeCell.closest("td")).toHaveClass("asb-code");
    expect(sizeCell.closest("tr")).toHaveTextContent("中继 B");
  });

  it("gives every header and body cell the shared left-alignment contract", () => {
    renderTable();

    const table = screen.getByRole("table", { name: "示例表格" });
    for (const cell of table.querySelectorAll("th, td")) {
      expect(cell).toHaveClass("asb-table-cell");
    }
  });

  it("appends the module-owned className next to the table contract", () => {
    renderTable({ className: "asb-runtime-log-table", rows: [] });

    expect(screen.getByRole("table", { name: "示例表格" })).toHaveClass(
      "asb-table",
      "asb-runtime-log-table",
    );
    expect(screen.getAllByRole("row")).toHaveLength(1);
  });
});
