import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import type { ConfigFileStatus } from "../api/client";
import { useConfigSnapshot } from "./useConfigSnapshot";

vi.mock("../api/client", () => ({
  getConfigStatus: vi.fn(),
  listProfiles: vi.fn(() => Promise.resolve([])),
  listBackups: vi.fn(() => Promise.resolve([])),
  getLockStatus: vi.fn(() => Promise.resolve({ state: "free" })),
  onTrayChanged: vi.fn(() => Promise.resolve(() => {})),
}));

describe("useConfigSnapshot active provider", () => {
  it.each(["codex", "claude"] as const)("uses %s live identity independently of full configuration matching", async (app) => {
    const status: ConfigFileStatus = {
      app, path: "/isolated/config", exists: true, syntaxOk: true,
      route: null, readError: null, lastSwitch: null,
      activeProfileId: `${app}-official`,
      matchStatus: { kind: "externallyModified", at: "2026-09-05T00:00:00Z" },
    };
    vi.mocked(client.getConfigStatus).mockResolvedValue([status]);
    const onError = vi.fn();
    const { result } = renderHook(() => useConfigSnapshot({ onError }));
    await waitFor(() => expect(result.current.activeProfileId(app)).toBe(`${app}-official`));
    expect(result.current.statuses?.[0].matchStatus.kind).toBe("externallyModified");
    expect(onError).not.toHaveBeenCalled();
  });

  it("does not reconstruct an active identity from a matching configuration", async () => {
    vi.mocked(client.getConfigStatus).mockResolvedValue([{
      app: "codex", path: "/isolated/config", exists: true, syntaxOk: true,
      route: null, readError: null, lastSwitch: null, activeProfileId: null,
      matchStatus: { kind: "matchesProfile", profileId: "historical", profileName: "Historical" },
    }]);
    const onError = vi.fn();
    const { result } = renderHook(() => useConfigSnapshot({ onError }));
    await waitFor(() => expect(result.current.statuses).not.toBeNull());
    expect(result.current.activeProfileId("codex")).toBeNull();
  });

  it("refreshes the main-window snapshot when a tray switch completes", async () => {
    const before: ConfigFileStatus = {
      app: "codex", path: "/isolated/config", exists: true, syntaxOk: true,
      route: null, readError: null, lastSwitch: null, activeProfileId: "codex-first",
      matchStatus: { kind: "matchesProfile", profileId: "codex-first", profileName: "第一档案" },
    };
    const after = {
      ...before,
      activeProfileId: "codex-second",
      matchStatus: { kind: "matchesProfile" as const, profileId: "codex-second", profileName: "第二档案" },
    };
    let changed: (() => void) | undefined;
    const stop = vi.fn();
    vi.mocked(client.getConfigStatus).mockResolvedValueOnce([before]).mockResolvedValue([after]);
    vi.mocked(client.onTrayChanged).mockImplementation(async (handler) => {
      changed = handler;
      return stop;
    });
    const onError = vi.fn();
    const { result, unmount } = renderHook(() => useConfigSnapshot({ onError }));

    await waitFor(() => expect(result.current.activeProfileId("codex")).toBe("codex-first"));
    changed?.();
    await waitFor(() => expect(result.current.activeProfileId("codex")).toBe("codex-second"));

    unmount();
    expect(stop).toHaveBeenCalledOnce();
  });
});
