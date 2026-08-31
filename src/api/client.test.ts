import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkCodexResetStatus,
  getCloudBackupSettings,
  getCloudBackupSetupSql,
  getAppSettings,
  getCachedCodexResetStatus,
  getConfigStatus,
  getGlobalPromptDocument,
  getSessionMessages,
  listSessions,
  listRuntimeLogs,
  listSystemFonts,
  openRuntimeLogDir,
  resetProfileStore,
  resumeSession,
  setAppSettings,
  setCloudBackupSettings,
  saveGlobalPromptDocument,
  uploadCloudBackup,
  restoreCloudBackup,
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

  it("reads application runtime records through their dedicated command", async () => {
    invokeMock.mockResolvedValue([]);

    await listRuntimeLogs();

    expect(invokeMock).toHaveBeenCalledWith("list_runtime_logs");
  });

  it("keeps global prompt paths in the backend-only document command contract", async () => {
    invokeMock.mockResolvedValue({});

    await getGlobalPromptDocument("codex");
    await saveGlobalPromptDocument("claude", "# Working agreement\n", "document-hash", true);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_global_prompt_document", {
      target: "codex",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "save_global_prompt_document", {
      target: "claude",
      content: "# Working agreement\n",
      expectedHash: "document-hash",
      confirmWrite: true,
    });
  });

  it("opens the app-owned runtime-log directory through its dedicated command", async () => {
    invokeMock.mockResolvedValue(undefined);

    await openRuntimeLogDir();

    expect(invokeMock).toHaveBeenCalledWith("open_runtime_log_dir");
  });

  it("propagates backend errors instead of inventing state", async () => {
    invokeMock.mockRejectedValue(new Error("backend unavailable"));
    await expect(getConfigStatus()).rejects.toThrow("backend unavailable");
  });

  it("reads public Codex reset signals through their dedicated command", async () => {
    invokeMock.mockResolvedValue({});

    await checkCodexResetStatus();

    expect(invokeMock).toHaveBeenCalledWith("check_codex_reset_status");
  });

  it("reads the cached Codex reset snapshot without using the network command", async () => {
    invokeMock.mockResolvedValue(null);

    await getCachedCodexResetStatus();

    expect(invokeMock).toHaveBeenCalledWith("get_cached_codex_reset_status");
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
      interfaceFont: "Noto Sans SC",
      runtimeLogLevel: "info" as const,
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

  it("lists installed system fonts for the interface-font picker", async () => {
    invokeMock.mockResolvedValue(["Segoe UI", "微软雅黑"]);

    await expect(listSystemFonts()).resolves.toEqual(["Segoe UI", "微软雅黑"]);

    expect(invokeMock).toHaveBeenCalledWith("list_system_fonts");
  });

  it("uses the dedicated encrypted-cloud-backup command contract", async () => {
    const settings = {
      projectUrl: "https://example.supabase.co",
      publishableKey: "sb_publishable_example",
      email: "backup@example.com",
    };
    invokeMock.mockResolvedValue(null);

    await getCloudBackupSettings();
    await setCloudBackupSettings(settings);
    await getCloudBackupSetupSql();
    await uploadCloudBackup("account-password", "backup-password", true);
    await restoreCloudBackup("account-password", "backup-password", true);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_cloud_backup_settings");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "set_cloud_backup_settings", { settings });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "cloud_backup_setup_sql");
    expect(invokeMock).toHaveBeenNthCalledWith(4, "upload_cloud_backup", {
      accountPassword: "account-password",
      backupPassword: "backup-password",
      confirmWrite: true,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(5, "restore_cloud_backup", {
      accountPassword: "account-password",
      backupPassword: "backup-password",
      confirmWrite: true,
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
