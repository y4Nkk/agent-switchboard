import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { UsageHistorySeries } from "../api/client";
import { ProviderUsageTrend } from "./ProviderUsageTrend";

describe("ProviderUsageTrend", () => {
  it("does not add independent plans merely because their declared unit matches", () => {
    const series: UsageHistorySeries[] = [
      {
        id: "primary",
        label: "主套餐余额",
        unit: "CNY",
        metric: "remaining",
        points: [{ at: "2026-09-01T00:00:00Z", value: 12.5 }],
      },
      {
        id: "add-on",
        label: "附加套餐余额",
        unit: "CNY",
        metric: "remaining",
        points: [{ at: "2026-09-02T00:00:00Z", value: 1_024.5 }],
      },
    ];

    render(<ProviderUsageTrend providerName="中继 A" series={series} loading={false} error={null} />);

    expect(screen.getByRole("figure", { name: "中继 A余额趋势（CNY）" })).toBeInTheDocument();
    expect(screen.queryByText("合计")).not.toBeInTheDocument();
    expect(screen.getAllByText("附加套餐余额").length).toBeGreaterThan(0);
    expect(screen.getByText("1,024.5 CNY")).toBeInTheDocument();
  });

  it("keeps a provider-owned tokens reading distinct from local model token formatting", () => {
    const series: UsageHistorySeries[] = [
      {
        id: "provider-tokens",
        label: "请求配额余额",
        unit: "tokens",
        metric: "remaining",
        points: [{ at: "2026-09-02T00:00:00Z", value: 225_700_972 }],
      },
    ];

    render(<ProviderUsageTrend providerName="中继 A" series={series} loading={false} error={null} />);

    expect(screen.getByText("225,700,972 tokens")).toBeInTheDocument();
    expect(screen.queryByText("225.7M tokens")).not.toBeInTheDocument();
  });
});
