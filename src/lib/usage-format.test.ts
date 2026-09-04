import { describe, expect, it } from "vitest";
import { formatCompactUsageValue, formatUsageValue } from "./usage-format";

describe("usage format", () => {
  it("keeps source decimals and units identical across usage surfaces", () => {
    expect(formatUsageValue(1_024.5, "CNY")).toBe("1,024.5 CNY");
    expect(formatUsageValue(12.5, "%")).toBe("12.5 %");
    expect(formatUsageValue(225_700_972, "tokens")).toBe("225,700,972 tokens");
  });

  it("uses the token compact format for axes while retaining non-token axis scale", () => {
    expect(formatCompactUsageValue(1_024.5)).toBe("1024.5");
  });
});
