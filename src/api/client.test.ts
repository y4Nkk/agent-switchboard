import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getAppSettings,
  getConfigStatus,
  getSessionMessages,
  listSessions,
  resetProfileStore,
  resumeSession,
  setAppSettings,
} from "./client";

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

  it("resets the app-owned profile store only after explicit confirmation", async () => {
    invokeMock.mockResolvedValue(undefined);

    await resetProfileStore(true);

    expect(invokeMock).toHaveBeenCalledWith("reset_profile_store", {
      confirmWrite: true,
    });
  });

  it("reads and saves application settings through their dedicated commands", async () => {
    const stored = {
      closeBehavior: "hideToTray" as const,
      theme: "system" as const,
      motion: "system" as const,
      alwaysOnTop: false,
      hardwareAcceleration: true,
    };
    const next = { ...stored, closeBehavior: "exit" as const, theme: "dark" as const };
    invokeMock.mockResolvedValue(stored);

    await expect(getAppSettings()).resolves.toEqual(stored);
    await expect(setAppSettings(next)).resolves.toEqual(stored);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_app_settings");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "set_app_settings", {
      settings: next,
    });
  });

  it("uses controlled session commands without a renderer-supplied path or command", async () => {
    invokeMock.mockResolvedValue({ sessions: [], issues: [] });

    await listSessions();
    await getSessionMessages("codex", "019f4b74-c859-7e72-bb0c-9f83347954fb");
    await resumeSession("claude", "019f4b74-c859-7e72-bb0c-9f83347954fb");

    expect(invokeMock).toHaveBeenNthCalledWith(1, "list_sessions");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_session_messages", {
      app: "codex",
      sessionId: "019f4b74-c859-7e72-bb0c-9f83347954fb",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "resume_session", {
      app: "claude",
      sessionId: "019f4b74-c859-7e72-bb0c-9f83347954fb",
    });
  });
});
