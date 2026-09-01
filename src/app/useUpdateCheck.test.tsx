import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { checkUpdate } from "../api/client";
import { useUpdateCheck } from "./useUpdateCheck";

vi.mock("../api/client", () => ({ checkUpdate: vi.fn() }));

const discoveredRelease = {
  currentVersion: "0.1.1",
  latestVersion: "v0.2.0",
  updateAvailable: true,
  releaseUrl: "https://github.com/y4Nkk/agent-switchboard/releases/tag/v0.2.0",
  checkedAt: "2026-09-01T00:00:00Z",
};

describe("useUpdateCheck", () => {
  beforeEach(() => {
    vi.mocked(checkUpdate).mockReset();
  });

  it("checks once on startup and keeps the discovered release for the update entry", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(discoveredRelease);
    const onError = vi.fn();
    const { result, rerender } = renderHook(() => useUpdateCheck({ onError }));

    await waitFor(() => expect(result.current.updateCheck).toEqual(discoveredRelease));
    rerender();

    expect(checkUpdate).toHaveBeenCalledTimes(1);
    expect(result.current.checking).toBe(false);
    expect(onError).not.toHaveBeenCalled();
  });

  it("keeps startup failures silent but reports a failed user retry", async () => {
    vi.mocked(checkUpdate).mockRejectedValueOnce(new Error("offline"));
    const onError = vi.fn();
    const { result } = renderHook(() => useUpdateCheck({ onError }));

    await waitFor(() => expect(checkUpdate).toHaveBeenCalledTimes(1));
    expect(onError).not.toHaveBeenCalled();

    vi.mocked(checkUpdate).mockRejectedValueOnce(new Error("offline"));
    await act(async () => {
      await result.current.runUpdateCheck();
    });

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: "offline" }));
  });
});
