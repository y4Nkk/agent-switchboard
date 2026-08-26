import { beforeEach, describe, expect, it, vi } from "vitest";
import { getConfigStatus } from "./client";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

describe("api client boundary", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("requests actual configuration status through the backend command", async () => {
    invokeMock.mockResolvedValue([]);

    await getConfigStatus();

    expect(invokeMock).toHaveBeenCalledWith("config_status");
  });

  it("propagates backend errors instead of inventing state", async () => {
    invokeMock.mockRejectedValue(new Error("backend unavailable"));
    await expect(getConfigStatus()).rejects.toThrow("backend unavailable");
  });
});
