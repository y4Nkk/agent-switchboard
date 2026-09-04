import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ModelUsageDistributionChart } from "./ModelUsageDistributionChart";

describe("ModelUsageDistributionChart", () => {
  it("keeps the five largest real models and names the summarized tail with exact values", () => {
    render(
      <ModelUsageDistributionChart
        ariaLabel="模型构成"
        emptyMessage="暂无模型记录"
        items={[
          { id: "one", label: "模型一", value: 70 },
          { id: "two", label: "模型二", value: 60 },
          { id: "three", label: "模型三", value: 50 },
          { id: "four", label: "模型四", value: 40 },
          { id: "five", label: "模型五", value: 30 },
          { id: "six", label: "模型六", value: 20 },
          { id: "seven", label: "模型七", value: 10 },
        ]}
      />,
    );

    expect(screen.getByRole("figure", { name: "模型构成" })).toBeInTheDocument();
    expect(screen.getByText("模型构成")).toBeInTheDocument();
    expect(screen.getByText("280")).toBeInTheDocument();
    expect(screen.getByText("模型一")).toBeInTheDocument();
    expect(screen.getByText("模型五")).toBeInTheDocument();
    expect(screen.queryByText("模型六")).not.toBeInTheDocument();
    const other = screen.getByText("其他（2 个模型）").closest("li");
    expect(other).toHaveTextContent("30 tokens");
    // The donut draws one sector per displayed slice.
    expect(document.querySelectorAll(".recharts-pie-sector")).toHaveLength(6);
  });

  it("uses the empty state when no positive finite model total exists", () => {
    render(
      <ModelUsageDistributionChart
        ariaLabel="模型构成"
        emptyMessage="暂无模型记录"
        items={[{ id: "missing", label: "未记录", value: Number.NaN }]}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("暂无模型记录");
  });
});
