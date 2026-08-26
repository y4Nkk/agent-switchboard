import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import * as client from "./api/client";
import type {
  CommonConfigPatch,
  ConfigFileStatus,
  FilePreview,
  ProviderProfile,
  SwitchLog,
} from "./api/client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const codexSwitch: SwitchLog = {
  app: "codex",
  profileId: "codex-gateway",
  profileName: "备用网关",
  contentHash: "hash-after",
  backupId: "b1",
  at: "2026-08-26T08:00:00Z",
};

const statuses: ConfigFileStatus[] = [
  {
    app: "codex",
    path: "C:/Users/test/.codex/config.toml",
    exists: true,
    syntaxOk: true,
    route: {
      app: "codex",
      routeMode: "custom",
      providerName: "本机网关",
      model: "gpt-5.3-codex",
      baseUrl: "https://gateway.internal/v1",
      envKey: "OPENAI_API_KEY",
      wireApi: "responses",
      codexModelOptions: null,
      haikuModel: null,
      sonnetModel: null,
      opusModel: null,
      availableModels: null,
      scopeWarnings: [],
    },
    readError: null,
    matchStatus: { kind: "externallyModified", at: "2026-08-26T08:00:00Z" },
    lastSwitch: codexSwitch,
  },
  {
    app: "claude",
    path: "C:/Users/test/.claude/settings.json",
    exists: true,
    syntaxOk: true,
    route: {
      app: "claude",
      routeMode: "official",
      providerName: null,
      model: "claude-sonnet-4",
      baseUrl: null,
      envKey: null,
      wireApi: null,
      codexModelOptions: null,
      haikuModel: null,
      sonnetModel: null,
      opusModel: null,
      availableModels: null,
      scopeWarnings: [],
    },
    readError: null,
    matchStatus: { kind: "unmanaged" },
    lastSwitch: null,
  },
];

const profiles: ProviderProfile[] = [
  {
    id: "codex-gateway",
    app: "codex",
    mode: "custom",
    name: "备用网关",
    model: "gpt-5.4",
    baseUrl: "https://backup.internal/v1",
    envKey: "OPENAI_API_KEY",
    modelOptions: null,
  },
];

const filePreview: FilePreview = {
  contentHash: "hash1",
  renderedHash: "rendered-hash1",
  preview: {
    app: "codex",
    target: "C:/Users/test/.codex/config.toml",
    changes: [{ key: "model", kind: "set", before: "gpt-5.3-codex", after: "gpt-5.4" }],
    preserved: ["threads"],
    warnings: [],
    backupDir: "C:/Users/test/AppData/Roaming/Agent Switchboard/state/backups",
  },
};

const emptyPatch = (app: "codex" | "claude"): CommonConfigPatch => ({ app, entries: [] });

function targetFrom(args: unknown): "codex" | "claude" {
  if (
    typeof args === "object" &&
    args !== null &&
    "target" in args &&
    (args as { target?: unknown }).target === "claude"
  ) {
    return "claude";
  }
  return "codex";
}

function primeBackend() {
  invokeMock.mockImplementation((command: string, args?: unknown) => {
    switch (command) {
      case "config_status":
        return Promise.resolve(statuses);
      case "list_profiles":
        return Promise.resolve(profiles);
      case "list_backups":
        return Promise.resolve([]);
      case "lock_status":
        return Promise.resolve({ state: "free" });
      case "get_common":
        return Promise.resolve(emptyPatch(targetFrom(args)));
      case "preview_switch":
        return Promise.resolve(filePreview);
      case "execute_switch":
        return Promise.resolve({
          lock: { state: "free" },
          acquiredAt: "2026-08-26T08:00:00Z",
          changed: ["C:/Users/test/.codex/config.toml"],
          warnings: [],
          backup: {
            id: "b1",
            app: "codex",
            targetPath: "C:/Users/test/.codex/config.toml",
            backupPath: "C:/backups/config.toml.bak",
            createdAt: "2026-08-26T08:00:00Z",
            contentHash: "h",
            targetExisted: true,
            reason: "switch",
          },
          preview: filePreview.preview,
          recovery: { outcome: "not_needed" },
          finalHash: "hash-after",
        });
      case "discover_local":
        return Promise.resolve({ codex: {}, claude: {}, importProposals: [] });
      default:
        return Promise.resolve([]);
    }
  });
}

describe("App integration with the typed client boundary", () => {
  it("loads actual status, renders lanes, and completes a confirmed switch", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByText("本机网关 · gpt-5.3-codex")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "供应商" }));
    await user.click(await screen.findByRole("option", { name: /备用网关/ }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("preview_switch", { profileId: "codex-gateway" }),
    );
    expect(await screen.findByText(/gpt-5\.3-codex → gpt-5\.4/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "安全切换" }));
    await user.click(await screen.findByRole("button", { name: "确认切换" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("execute_switch", {
        profileId: "codex-gateway",
        expectedHash: "hash1",
        expectedRenderedHash: "rendered-hash1",
        confirmWrite: true,
      }),
    );
    expect(await screen.findByText(/已完成切换/)).toBeInTheDocument();
  });

  it("reports match state, last switch time, and the user-config scope on the overview", async () => {
    primeBackend();
    render(<App />);

    expect(await screen.findByText(/与上次切换 .* 不符，配置可能被外部修改/)).toBeInTheDocument();
    expect(await screen.findByText(/上次切换 2026-08-26 08:00:00/)).toBeInTheDocument();
    expect(screen.getByText(/仅管理用户级配置/)).toBeInTheDocument();
    expect(screen.getAllByText("自定义服务 · gpt-5.3-codex").length).toBeGreaterThan(0);
    expect(screen.getAllByText("官方登录 · claude-sonnet-4").length).toBeGreaterThan(0);
  });

  it("undoes the last switch through an explicit confirmation", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "config_status") return Promise.resolve(statuses);
      if (command === "list_profiles") return Promise.resolve(profiles);
      if (command === "list_backups") return Promise.resolve([]);
      if (command === "lock_status") return Promise.resolve({ state: "free" });
      if (command === "get_common") return Promise.resolve(emptyPatch(targetFrom(args)));
      if (command === "undo_last_switch") {
        return Promise.resolve({
          preRestoreBackup: {
            id: "restore-b1",
            app: "codex",
            targetPath: "C:/Users/test/.codex/config.toml",
            backupPath: "C:/backups/config.toml.restore.bak",
            createdAt: "2026-08-26T08:01:00Z",
            contentHash: "hash-after",
            targetExisted: true,
            reason: "restore-precheck",
          },
          restoredHash: "hash-before",
          warnings: [],
        });
      }
      return Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "备份" }));
    await user.click(await screen.findByRole("button", { name: "撤回上一次切换" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认撤回" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("undo_last_switch", {
        target: "codex",
        confirmWrite: true,
      }),
    );
  });

  it("invalidates an existing preview after a common setting changes", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "供应商" }));
    await user.click(await screen.findByRole("option", { name: /备用网关/ }));
    expect(await screen.findByText(/gpt-5\.3-codex → gpt-5\.4/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "通用设置" }));
    await user.click(await screen.findByLabelText("禁用响应存储"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_common", {
        target: "codex",
        patch: { app: "codex", entries: [{ key: "disable_response_storage", value: true }] },
      }),
    );

    expect(screen.getByRole("button", { name: "安全切换" })).toBeDisabled();
  });

  it("shows a local read failure instead of treating it as a missing file", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "config_status") return Promise.resolve(statuses);
      if (command === "list_profiles") return Promise.resolve(profiles);
      if (command === "list_backups") return Promise.resolve([]);
      if (command === "lock_status") return Promise.resolve({ state: "free" });
      if (command === "get_common") return Promise.resolve(emptyPatch("codex"));
      if (command === "discover_local") {
        return Promise.resolve({
          codex: {
            app: "codex",
            path: "C:/Users/test/.codex/config.toml",
            exists: true,
            state: { kind: "readError", message: "无法读取配置文件" },
          },
          claude: {
            app: "claude",
            path: "C:/Users/test/.claude/settings.json",
            exists: false,
            state: { kind: "missing" },
          },
          importProposals: [],
        });
      }
      return Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "发现" }));
    await user.click(screen.getByRole("button", { name: "扫描配置" }));
    expect(await screen.findByText("无法读取配置文件")).toBeInTheDocument();
  });

  it("surfaces typed command errors instead of hiding them", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "config_status") {
        return Promise.reject({ code: "read-current", message: "无法读取当前配置" });
      }
      return Promise.resolve([]);
    });
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取当前配置");
  });
});

describe("client boundary", () => {
  it("re-exports commands as typed functions only", () => {
    for (const exported of Object.keys(client)) {
      expect(typeof client[exported as keyof typeof client]).toBe("function");
    }
  });
});
