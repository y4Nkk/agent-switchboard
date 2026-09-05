import { describe, expect, it } from "vitest";
import { formatCompactUsageValue, formatUsageValue, formatUsageSummary } from "./usage-format";

describe("usage format", () => {
  it("summarizes each plan without adding balances or inventing percentages", () => {
    expect(formatUsageSummary({ at: "2026-09-05T08:00:00Z", readings: [
      { planName: "主套餐", remaining: 30, used: 10, total: 40, unit: "CNY" },
      { planName: "附加套餐", remaining: 8, used: null, total: null, unit: "USD" },
      { remaining: null, used: 25, total: 100, unit: "次" },
    ] })).toBe("主套餐：剩余 75% · 余额 30 CNY；附加套餐：余额 8 USD；额度 3：剩余 75%");
  });

  it("handles empty, exhausted, overdrawn and unknown quotas", () => {
    const format = (remaining: number | null, used: number | null, total: number | null) =>
      formatUsageSummary({ at: "2026-09-05T08:00:00Z", readings: [{ remaining, used, total, unit: "CNY" }] });
    expect(formatUsageSummary({ at: "2026-09-05T08:00:00Z", readings: [] })).toBe("暂无额度读数");
    expect(format(0, null, 100)).toBe("剩余 0% · 余额 0 CNY");
    expect(format(-5, null, 100)).toBe("剩余 -5% · 余额 -5 CNY");
    expect(format(0, null, 0)).toBe("余额 0 CNY");
    expect(format(null, 4, null)).toBe("已用 4 CNY");
    expect(format(null, null, null)).toBe("暂无额度读数");
  });
  it("keeps source decimals and units identical across usage surfaces", () => {
    expect(formatUsageValue(1_024.5, "CNY")).toBe("1,024.5 CNY");
    expect(formatUsageValue(12.5, "%")).toBe("12.5 %");
    expect(formatUsageValue(225_700_972, "tokens")).toBe("225,700,972 tokens");
  });

  it("uses the token compact format for axes while retaining non-token axis scale", () => {
    expect(formatCompactUsageValue(1_024.5)).toBe("1024.5");
  });
});
