import { describe, expect, it } from "vitest";
import {
  TOKEN_UNIT,
  formatCompactTokenCount,
  formatTokenCount,
  formatTokenValue,
} from "./token-format";

describe("token format", () => {
  it("uses K, M, and B for compact token quantities", () => {
    expect(formatCompactTokenCount(789_241)).toBe("789.2K");
    expect(formatCompactTokenCount(12_997_939)).toBe("13M");
    expect(formatCompactTokenCount(225_700_972)).toBe("225.7M");
    expect(formatCompactTokenCount(1_300_000_000)).toBe("1.3B");
    expect(formatCompactTokenCount(999_999)).toBe("1M");
  });

  it("keeps exact values for detail and makes compact values self-describing", () => {
    expect(formatTokenCount(225_700_972)).toBe("225,700,972");
    expect(formatTokenValue(225_700_972)).toBe("225.7M tokens");
    expect(TOKEN_UNIT).toBe("tokens");
  });

});
