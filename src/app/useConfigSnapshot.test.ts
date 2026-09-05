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
});
