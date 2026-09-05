import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Update } from "@tauri-apps/plugin-updater";
import {
  checkUpdate,
  closeUpdate,
  getUpdateChannel,
  installUpdate,
  restartApplication,
  type UpdateCheck,
} from "../api/client";
import { useUpdateCheck } from "./useUpdateCheck";

vi.mock("../api/client", () => ({
  checkUpdate: vi.fn(),
  closeUpdate: vi.fn(),
  getUpdateChannel: vi.fn(),
  installUpdate: vi.fn(),
  restartApplication: vi.fn(),
}));

const nativeUpdate = {} as Update;
const discoveredUpdate: UpdateCheck = {
  currentVersion: "0.1.1",
  latestVersion: "0.2.0",
  releaseNotes: null,
  checkedAt: "2026-09-01T00:00:00Z",
  update: nativeUpdate,
};

describe("useUpdateCheck", () => {
  beforeEach(() => {
    vi.mocked(checkUpdate).mockReset();
    vi.mocked(closeUpdate).mockReset();
    vi.mocked(getUpdateChannel).mockReset();
    vi.mocked(installUpdate).mockReset();
    vi.mocked(restartApplication).mockReset();
    vi.mocked(closeUpdate).mockResolvedValue();
    vi.mocked(getUpdateChannel).mockResolvedValue("github");
    vi.mocked(restartApplication).mockResolvedValue();
  });

  it("checks once on startup and keeps a signed update for installation", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(discoveredUpdate);
    const onError = vi.fn();
    const { result, rerender } = renderHook(() => useUpdateCheck({ onError }));

    await waitFor(() => expect(result.current.updateCheck).toEqual(discoveredUpdate));
    rerender();

    expect(checkUpdate).toHaveBeenCalledTimes(1);
    expect(result.current.lastCheckedAt).toBe(discoveredUpdate.checkedAt);
    expect(result.current.checking).toBe(false);
    expect(onError).not.toHaveBeenCalled();
  });

  it("records an up-to-date check without retaining an update resource", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(null);
    const { result } = renderHook(() => useUpdateCheck({ onError: vi.fn() }));

    await waitFor(() => expect(result.current.lastCheckedAt).not.toBeNull());

    expect(result.current.updateCheck).toBeNull();
    expect(closeUpdate).not.toHaveBeenCalled();
  });

  it("leaves a Store installation to Microsoft Store updates", async () => {
    vi.mocked(getUpdateChannel).mockResolvedValue("microsoftStore");
    const { result } = renderHook(() => useUpdateCheck({ onError: vi.fn() }));

    await waitFor(() => expect(result.current.updateChannel).toBe("microsoftStore"));

    expect(checkUpdate).not.toHaveBeenCalled();
    expect(result.current.lastCheckedAt).toBeNull();
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

  it("streams download progress, releases the update, and restarts after installation", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(discoveredUpdate);
    vi.mocked(installUpdate).mockImplementation(async (_update, onEvent) => {
      onEvent({ event: "Started", data: { contentLength: 4096 } });
      onEvent({ event: "Progress", data: { chunkLength: 1024 } });
    });
    const { result } = renderHook(() => useUpdateCheck({ onError: vi.fn() }));

    await waitFor(() => expect(result.current.updateCheck).toEqual(discoveredUpdate));
    await act(async () => {
      await result.current.installAvailableUpdate();
    });

    expect(installUpdate).toHaveBeenCalledWith(nativeUpdate, expect.any(Function));
    expect(closeUpdate).toHaveBeenCalledWith(nativeUpdate);
    expect(restartApplication).toHaveBeenCalledTimes(1);
    expect(result.current.restartRequired).toBe(true);
  });

  it("keeps a failed download available for a direct retry", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(discoveredUpdate);
    vi.mocked(installUpdate).mockRejectedValueOnce(new Error("signature invalid"));
    const onError = vi.fn();
    const { result } = renderHook(() => useUpdateCheck({ onError }));

    await waitFor(() => expect(result.current.updateCheck).toEqual(discoveredUpdate));
    await act(async () => {
      await result.current.installAvailableUpdate();
    });

    expect(result.current.updateCheck).toEqual(discoveredUpdate);
    expect(result.current.installing).toBe(false);
    expect(result.current.downloadProgress).toBeNull();
    expect(closeUpdate).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: "signature invalid" }));

    vi.mocked(installUpdate).mockResolvedValueOnce();
    await act(async () => {
      await result.current.installAvailableUpdate();
    });

    expect(installUpdate).toHaveBeenCalledTimes(2);
    expect(closeUpdate).toHaveBeenCalledWith(nativeUpdate);
    expect(restartApplication).toHaveBeenCalledTimes(1);
  });

  it("keeps a completed installation in restart-required state when relaunch fails", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(discoveredUpdate);
    vi.mocked(installUpdate).mockResolvedValue();
    vi.mocked(restartApplication).mockRejectedValueOnce(new Error("restart failed"));
    const onError = vi.fn();
    const { result } = renderHook(() => useUpdateCheck({ onError }));

    await waitFor(() => expect(result.current.updateCheck).toEqual(discoveredUpdate));
    await act(async () => {
      await result.current.installAvailableUpdate();
    });

    expect(result.current.restartRequired).toBe(true);
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: "restart failed" }));

    await act(async () => {
      await result.current.restartInstalledUpdate();
    });
    expect(restartApplication).toHaveBeenCalledTimes(2);
  });

  it("releases a discovered update when the consumer unmounts", async () => {
    vi.mocked(checkUpdate).mockResolvedValue(discoveredUpdate);
    const { result, unmount } = renderHook(() => useUpdateCheck({ onError: vi.fn() }));

    await waitFor(() => expect(result.current.updateCheck).toEqual(discoveredUpdate));
    unmount();

    expect(closeUpdate).toHaveBeenCalledWith(nativeUpdate);
  });
});
