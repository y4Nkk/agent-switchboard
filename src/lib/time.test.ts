import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { countdownLabel } from "./time";

describe("countdownLabel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-02T08:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports a passed reset time without a countdown", () => {
    expect(countdownLabel("2026-09-02T07:59:59Z")).toBe("已到重置时间");
    expect(countdownLabel("2026-09-02T08:00:00Z")).toBe("已到重置时间");
  });

  it("counts days with remaining hours once a day is left", () => {
    expect(countdownLabel("2026-09-06T08:00:00Z")).toBe("约 4 天 0 小时后");
    expect(countdownLabel("2026-09-03T11:30:00Z")).toBe("约 1 天 3 小时后");
  });

  it("counts hours with remaining minutes below one day", () => {
    expect(countdownLabel("2026-09-02T13:05:00Z")).toBe("约 5 小时 5 分钟后");
  });

  it("counts bare minutes below one hour", () => {
    expect(countdownLabel("2026-09-02T08:42:00Z")).toBe("约 42 分钟后");
  });
});
