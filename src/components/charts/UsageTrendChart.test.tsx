import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { formatChartValue, prepareTrendSeries, trendYAxisDomain } from "./chart-data";
import { UsageTrendChart } from "./UsageTrendChart";

describe("UsageTrendChart", () => {
  it("headlines the summed newest reading and legends every series", () => {
    render(
      <UsageTrendChart
        ariaLabel="本地 token 趋势"
        emptyMessage="暂无记录"
        title="每日 Token 趋势"
        valueKind="local-token"
        series={[
          {
            id: "fresh",
            label: "新输入",
            unit: "tokens",
            points: [
              { at: "2026-09-01T00:00:00Z", value: 10 },
              { at: "2026-09-02T00:00:00Z", value: 20 },
            ],
          },
          {
            id: "output",
            label: "输出",
            unit: "tokens",
            points: [{ at: "2026-09-02T00:00:00Z", value: 5 }],
          },
        ]}
      />,
    );

    expect(screen.getByRole("figure", { name: "本地 token 趋势" })).toBeInTheDocument();
    expect(screen.getByText("每日 Token 趋势")).toBeInTheDocument();
    // Rest headline sums both series at the newest real timestamp: 20 + 5.
    expect(screen.getByText("合计")).toBeInTheDocument();
    expect(screen.getByText("25 tokens")).toBeInTheDocument();
    expect(screen.getByText("单位：tokens")).toBeInTheDocument();
    expect(screen.getByText("新输入")).toBeInTheDocument();
    expect(screen.getByText("输出")).toBeInTheDocument();
    // Both series draw on the shared timestamp axis; no invented points exist.
    const curves = document.querySelectorAll(".recharts-line-curve");
    expect(curves).toHaveLength(2);
    expect(curves[0].getAttribute("stroke")).toContain("--color-chart-");
    expect(screen.getByRole("figure", { name: "本地 token 趋势" })).toHaveClass("rounded-3xl");
  });

  it("does not plot malformed points or combine incompatible units", () => {
    const { rerender } = render(
      <UsageTrendChart
        ariaLabel="余额趋势"
        emptyMessage="没有余额快照"
        valueKind="generic"
        series={[{ id: "bad", label: "余额", points: [{ at: "not-a-time", value: Number.NaN }] }]}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("没有余额快照");

    rerender(
      <UsageTrendChart
        ariaLabel="余额趋势"
        emptyMessage="没有余额快照"
        valueKind="generic"
        series={[
          { id: "currency", label: "余额", unit: "CNY", points: [{ at: "2026-09-01T00:00:00Z", value: 1 }] },
          { id: "calls", label: "请求", unit: "次", points: [{ at: "2026-09-01T00:00:00Z", value: 2 }] },
        ]}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("不同单位");
  });

  it("tracks the first series when sums are not meaningful", () => {
    render(
      <UsageTrendChart
        ariaLabel="官方额度趋势"
        emptyMessage="暂无官方额度快照"
        valueKind="percentage"
        series={[{ id: "weekly", label: "7 天", unit: "%", points: [{ at: "2026-09-01T00:00:00Z", value: 62 }] }]}
      />,
    );

    expect(screen.getByRole("figure", { name: "官方额度趋势" })).toBeInTheDocument();
    expect(screen.getAllByText("7 天").length).toBe(2); // headline label + legend entry
    expect(screen.getByText("62 %")).toBeInTheDocument();
    // A lone recorded value is drawn as a visible dot, not a second point.
    expect(document.querySelectorAll(".recharts-line-dot")).toHaveLength(1);
    expect(document.querySelectorAll(".recharts-line-curve")).toHaveLength(0);
  });

  it("keeps a fractional newest reading when series timestamps do not align", () => {
    render(
      <UsageTrendChart
        ariaLabel="多套餐余额趋势"
        emptyMessage="暂无"
        valueKind="generic"
        series={[
          { id: "primary", label: "主套餐余额", unit: "CNY", points: [{ at: "2026-09-01T00:00:00Z", value: 12.5 }] },
          { id: "add-on", label: "附加套餐余额", unit: "CNY", points: [{ at: "2026-09-02T00:00:00Z", value: 1_024.5 }] },
        ]}
      />,
    );

    expect(screen.getAllByText("附加套餐余额").length).toBeGreaterThan(0);
    expect(screen.getByText("1,024.5 CNY")).toBeInTheDocument();
    expect(screen.queryByText("0 CNY")).not.toBeInTheDocument();
  });

  it("uses a fixed percentage range and keeps negative source values visible", () => {
    const percentage = prepareTrendSeries([
      { id: "quota", label: "7 天", unit: "%", points: [{ at: "2026-09-01T00:00:00Z", value: 76.25 }] },
    ]);
    const negative = prepareTrendSeries([
      { id: "credit", label: "余额", unit: "USD", points: [{ at: "2026-09-01T00:00:00Z", value: -5 }] },
    ]);

    expect(trendYAxisDomain(percentage, "percentage")).toEqual([0, 100]);
    expect(trendYAxisDomain(negative, "generic")).toEqual([-5.5, 0]);
  });

  it("shrinks into the compact variant for embedded panels", () => {
    render(
      <UsageTrendChart
        ariaLabel="余额趋势"
        emptyMessage="暂无"
        size="compact"
        valueKind="generic"
        series={[{ id: "balance", label: "余额", unit: "CNY", points: [{ at: "2026-09-01T00:00:00Z", value: 1024 }] }]}
      />,
    );

    const card = screen.getByRole("figure", { name: "余额趋势" });
    expect(card.className).toContain("h-56");
    expect(card.className).not.toContain("h-[344px]");
    // The compact headline steps down one title level.
    expect(card.querySelector(".text-title-2-medium")).not.toBeNull();
    expect(card.querySelector(".text-title-1-medium")).toBeNull();
  });

  it("uses explicit source semantics instead of inferring local tokens from a unit", () => {
    const value = 225_700_972;

    expect(formatChartValue(value, "tokens", "generic")).toBe("225,700,972 tokens");
    expect(formatChartValue(value, "tokens", "local-token")).toBe("225.7M tokens");
  });
});
