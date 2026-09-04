import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { UsageTrendChart } from "./UsageTrendChart";

describe("UsageTrendChart", () => {
  it("renders real points in their supplied order with text legend and keyboard detail", () => {
    render(
      <UsageTrendChart
        ariaLabel="本地 token 趋势"
        emptyMessage="暂无记录"
        series={[
          {
            id: "gpt",
            label: "一个很长很长很长很长很长的模型名称",
            unit: "tokens",
            points: [
              { at: "2026-09-02T00:00:00Z", value: 20 },
              { at: "2026-09-01T00:00:00Z", value: 10 },
            ],
          },
        ]}
      />,
    );

    expect(screen.getByRole("group", { name: "本地 token 趋势" })).toBeInTheDocument();
    const firstPoint = screen.getByLabelText(/20 tokens/);
    expect(firstPoint).toHaveAttribute("tabindex", "0");
    expect(screen.getByTitle("一个很长很长很长很长很长的模型名称")).toBeInTheDocument();

    fireEvent.focus(firstPoint);
    expect(screen.getByRole("status")).toHaveTextContent("2026年09月02日 08:00");
    expect(screen.getByRole("status")).toHaveTextContent("20 tokens");

    const path = document.querySelector(".asb-chart-line")?.getAttribute("d") ?? "";
    const coordinates = [...path.matchAll(/[ML]([\d.]+) ([\d.]+)/g)].map((match) => Number(match[1]));
    expect(coordinates[0]).toBeGreaterThan(coordinates[1] ?? Number.POSITIVE_INFINITY);
  });

  it("does not plot malformed points or combine incompatible units", () => {
    const { rerender } = render(
      <UsageTrendChart
        ariaLabel="余额趋势"
        emptyMessage="没有余额快照"
        series={[{ id: "bad", label: "余额", points: [{ at: "not-a-time", value: Number.NaN }] }]}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("没有余额快照");

    rerender(
      <UsageTrendChart
        ariaLabel="余额趋势"
        emptyMessage="没有余额快照"
        series={[
          { id: "currency", label: "余额", unit: "CNY", points: [{ at: "2026-09-01T00:00:00Z", value: 1 }] },
          { id: "calls", label: "请求", unit: "次", points: [{ at: "2026-09-01T00:00:00Z", value: 2 }] },
        ]}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("不同单位");
  });

  it("supports a single real point without manufacturing a second one", () => {
    render(
      <UsageTrendChart
        ariaLabel="官方额度趋势"
        emptyMessage="暂无官方额度快照"
        series={[{ id: "weekly", label: "7 天", unit: "%", points: [{ at: "2026-09-01T00:00:00Z", value: 62 }] }]}
      />,
    );

    expect(screen.getAllByLabelText(/7 天，62 %/)).toHaveLength(1);
    expect(document.querySelectorAll(".asb-chart-point")).toHaveLength(1);
  });
});
