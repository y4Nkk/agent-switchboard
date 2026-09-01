import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkCodexResetStatus,
  deleteProfile,
  getCommonSettingsEditor,
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
  reorderProfiles,
  resumeSession,
  restartApplication,
  setAppSettings,
  setCloudBackupSettings,
  saveGlobalPromptDocument,
  saveCommonSettings,
  previewCommonSettings,
  queryProfileUsage,
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

  it("reads, previews, and saves application-owned general settings without a client-file write command", async () => {
    invokeMock.mockResolvedValue({});

    await getCommonSettingsEditor("codex");
    await saveCommonSettings(
      "codex",
      { settings: { disable_response_storage: true } },
      "settings-hash",
    );
    await previewCommonSettings("codex", { settings: { disable_response_storage: true } });

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_common_settings_editor", {
      target: "codex",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "save_common_settings", {
      target: "codex",
      settings: { settings: { disable_response_storage: true } },
      expectedSettingsHash: "settings-hash",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "preview_common_settings", {
      target: "codex",
      settings: { settings: { disable_response_storage: true } },
    });
  });

  it("sends the current provider-file revisions for delete and reorder", async () => {
    invokeMock.mockResolvedValue([]);

    await deleteProfile("codex-gateway", "delete-hash");
    await reorderProfiles(
      "codex",
      ["codex-b", "codex-a"],
      { "codex-a": "hash-a", "codex-b": "hash-b" },
    );

    expect(invokeMock).toHaveBeenNthCalledWith(1, "delete_profile", {
      profileId: "codex-gateway",
      expectedFileHash: "delete-hash",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "reorder_profiles", {
      target: "codex",
      orderedIds: ["codex-b", "codex-a"],
      expectedFileHashes: { "codex-a": "hash-a", "codex-b": "hash-b" },
    });
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

  it("queries a persisted provider usage summary without passing its credential to the UI", async () => {
    invokeMock.mockResolvedValue({ readings: [], at: "2026-09-01T08:00:00Z" });

    await queryProfileUsage("codex-relay-a");

    expect(invokeMock).toHaveBeenCalledWith("query_profile_usage", {
      profileId: "codex-relay-a",
    });
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
      launchAtLogin: false,
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

  it("restarts the native application through its dedicated command", async () => {
    invokeMock.mockResolvedValue(undefined);

    await expect(restartApplication()).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("restart_application");
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
