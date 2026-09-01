import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import * as client from "./api/client";
import type {
  ConfigWriteRecord,
  ConfigFileStatus,
  FilePreview,
  ProviderRecord,
  RuntimeLogEntry,
} from "./api/client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onResized: () => Promise.resolve(() => {}) }),
}));
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const codexSwitch: ConfigWriteRecord = {
  app: "codex",
  profileId: "codex-gateway",
  profileName: "备用网关",
  contentHash: "hash-after",
  backupId: "b1",
  at: "2026-08-26T08:00:00Z",
  operation: "projection",
};

const runtimeLogs: RuntimeLogEntry[] = [
  {
    at: "2026-08-26T08:00:00Z",
    level: "info",
    action: "configurationSwitched",
  },
];

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
      apiKey: "OPENAI_API_KEY",
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
      apiKey: "test-api-key",
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

const profiles: ProviderRecord[] = [
  {
    profile: {
      id: "codex-gateway",
      app: "codex",
      routeMode: "custom",
      name: "备用网关",
      model: "gpt-5.4",
      baseUrl: "https://backup.internal/v1",
      apiKey: "OPENAI_API_KEY",
      modelOptions: null,
      websiteUrl: null,
    },
    fileHash: "provider-file-hash",
  },
];

const filePreview: FilePreview = {
  contentHash: "hash1",
  renderedHash: "rendered-hash1",
  content: 'model = "gpt-5.4"\nthreads = 8\n',
  preview: {
    app: "codex",
    target: "C:/Users/test/.codex/config.toml",
    changes: [{ key: "model", kind: "set", before: "gpt-5.3-codex", after: "gpt-5.4" }],
    warnings: [],
    backupDir: "C:/Users/test/AppData/Roaming/Agent Switchboard/state/backups",
  },
};

const defaultSettings = {
  closeBehavior: "hideToTray",
  theme: "system",
  motion: "system",
  alwaysOnTop: false,
  launchAtLogin: false,
  hardwareAcceleration: true,
  interfaceFont: "Noto Sans SC",
  runtimeLogLevel: "info",
};

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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

function primeBackend(logEntries: RuntimeLogEntry[] = []) {
  invokeMock.mockImplementation((command: string, args?: unknown) => {
    switch (command) {
      case "config_status":
        return Promise.resolve(statuses);
      case "list_profiles":
        return Promise.resolve(profiles);
      case "list_backups":
        return Promise.resolve([]);
      case "list_runtime_logs":
        return Promise.resolve(logEntries);
      case "lock_status":
        return Promise.resolve({ state: "free" });
      case "get_app_settings":
        return Promise.resolve(defaultSettings);
      case "set_app_settings":
        return Promise.resolve((args as { settings: unknown }).settings);
      case "list_system_fonts":
        return Promise.resolve(["Microsoft YaHei", "Noto Sans SC"]);
      case "check_update":
        return Promise.resolve({
          currentVersion: "0.1.1",
          latestVersion: "0.1.1",
          updateAvailable: false,
          releaseUrl: "https://github.com/y4Nkk/agent-switchboard/releases/tag/v0.1.1",
          checkedAt: "2026-09-01T00:00:00Z",
        });
      case "get_common_settings_editor":
        return Promise.resolve({
          app: targetFrom(args),
          settings: { settings: { hide_agent_reasoning: false } },
          settingsHash: `${targetFrom(args)}-settings-hash`,
          groups: ["模型行为", "安全与审批", "隐私与数据"],
          specs: [
            {
              key: "hide_agent_reasoning",
              label: "隐藏推理摘要",
              group: "模型行为",
              control: "toggle",
              default: { boolValue: false },
              options: [],
            },
          ],
          directory: [],
        });
      case "save_common_settings":
        return Promise.resolve({
          settings: (args as { settings: unknown }).settings,
          settingsHash: "saved-settings-hash",
        });
      case "get_global_prompt_document": {
        const target = targetFrom(args);
        return Promise.resolve({
          app: target,
          fileName: target === "codex" ? "AGENTS.md" : "CLAUDE.md",
          content: target === "codex" ? "# Codex global instructions\n" : "# Claude global instructions\n",
          contentHash: `${target}-prompt-hash`,
          exists: true,
        });
      }
      case "save_global_prompt_document": {
        const target = targetFrom(args);
        const payload = args as { content: string };
        return Promise.resolve({
          app: target,
          fileName: target === "codex" ? "AGENTS.md" : "CLAUDE.md",
          content: payload.content,
          contentHash: `${target}-saved-prompt-hash`,
          exists: true,
        });
      }
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
      case "window_is_maximized":
        return Promise.resolve(false);
      default:
        return Promise.resolve([]);
    }
  });
}

const ccScan = {
  dbPath: "C:/Users/test/.cc-switch/cc-switch.db",
  providers: [
    {
      key: "claude:id-1",
      app: "claude",
      routeMode: "custom",
      name: "中继 A",
      model: "claude-x",
      baseUrl: "https://relay.internal",
      usageScriptImportable: true,
      usageScriptUpdatesExisting: false,
      warnings: [],
      existing: false,
    },
    {
      key: "codex:id-2",
      app: "codex",
      routeMode: "official",
      name: "Codex 官方登录",
      model: null,
      baseUrl: null,
      usageScriptImportable: false,
      usageScriptUpdatesExisting: false,
      warnings: [],
      existing: false,
    },
  ],
  skipped: [
    { key: "gemini:id-3", appType: "gemini", name: "双子", reason: "客户端 gemini 超出本应用支持范围" },
  ],
};

describe("App integration with the typed client boundary", () => {
  it("loads actual status, renders lanes, and completes a confirmed switch", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByText("本机网关")).toBeInTheDocument());
    expect(screen.getByText("gpt-5.3-codex")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "供应商" }));
    await user.click(await screen.findByRole("option", { name: /备用网关/ }));
    // Selection alone shows no diff; the diff appears on explicit request.
    expect(screen.queryByRole("region", { name: "变更预览" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "预览 备用网关 变更" }));
    const previewPanel = await screen.findByRole("region", { name: "变更预览" });
    expect(within(previewPanel).getByText("gpt-5.3-codex")).toBeInTheDocument();
    expect(within(previewPanel).getByText("gpt-5.4")).toBeInTheDocument();

    // The preview unfolds under the provider list, inside the same panel,
    // and the eye button retracts it (user decision 2026-08-28).
    expect(screen.getByRole("region", { name: "供应商工作区" })).toContainElement(previewPanel);
    await user.click(screen.getByRole("button", { name: "收起 备用网关 预览" }));
    expect(screen.queryByRole("region", { name: "变更预览" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "预览 备用网关 变更" }));
    await screen.findByRole("region", { name: "变更预览" });

    // The switch affordance lives on the overview only.
    await user.click(screen.getByRole("button", { name: "概览" }));
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
    expect(await screen.findByText(/已切换到「备用网关」/)).toBeInTheDocument();
  });

  it("opens the logs tab through the typed application-log command", async () => {
    primeBackend(runtimeLogs);
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText("本机网关");
    await user.click(screen.getByRole("button", { name: "日志" }));

    expect(await screen.findByText("已切换配置")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_runtime_logs");
  });

  it("saves the selected runtime-log threshold through the one app-settings path", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "日志" }));
    await screen.findByText("暂无应用运行日志");
    const levelControl = screen.getByRole("combobox", { name: "记录级别" });
    await waitFor(() => expect(levelControl).not.toBeDisabled());
    await user.click(levelControl);
    await user.click(screen.getByRole("option", { name: "静默" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
        settings: {
          ...defaultSettings,
          runtimeLogLevel: "silent",
        },
      }),
    );
  });

  it("reports match state, last switch time, and the user-config scope on the overview", async () => {
    primeBackend();
    render(<App />);

    expect(await screen.findByText(/与上次切换 .* 不符，配置可能被外部修改/)).toBeInTheDocument();
    const lastSwitchRow = screen.getByText("上次切换").closest(".asb-status-row");
    expect(lastSwitchRow).toHaveTextContent("2026-08-26 16:00:00");
    expect(screen.getAllByText("本机网关 · gpt-5.3-codex").length).toBeGreaterThan(0);
    expect(screen.getAllByText("官方登录 · claude-sonnet-4").length).toBeGreaterThan(0);
  });

  it("undoes the last switch through an explicit confirmation", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "config_status") return Promise.resolve(statuses);
      if (command === "list_profiles") return Promise.resolve(profiles);
      if (command === "list_backups") return Promise.resolve([]);
      if (command === "lock_status") return Promise.resolve({ state: "free" });
      if (command === "get_app_settings") return Promise.resolve(defaultSettings);
      if (command === "backup_diff") {
        return Promise.resolve([
          { key: "model", kind: "set", before: "gpt-5.3", after: "gpt-5.4" },
        ]);
      }
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
    const dialog = await screen.findByRole("dialog", { name: "撤回上一次切换" });
    const diff = await screen.findByLabelText("撤回后写入的差异");
    expect(within(diff).getByText("gpt-5.4")).toHaveClass("asb-diff-old");
    expect(within(diff).getByText("gpt-5.3")).toHaveClass("asb-diff-new");
    expect(within(dialog).getByRole("button", { name: "确认撤回" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "确认撤回" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("undo_last_switch", {
        target: "codex",
        confirmWrite: true,
      }),
    );
    expect(await screen.findByText("已撤回上一次切换")).toBeInTheDocument();
  });

  it("keeps undo confirmation unavailable until the exact difference is ready", async () => {
    primeBackend();
    const pendingDiff = deferred<Awaited<ReturnType<typeof client.backupDiff>>>();
    const backend = invokeMock.getMockImplementation();
    expect(backend).toBeDefined();
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "backup_diff") return pendingDiff.promise;
      return backend!(command, args as never);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "备份" }));
    await user.click(await screen.findByRole("button", { name: "撤回上一次切换" }));
    const dialog = await screen.findByRole("dialog", { name: "撤回上一次切换" });
    expect(within(dialog).getByRole("button", { name: "确认撤回" })).toBeDisabled();
    expect(within(dialog).getByText("正在生成撤回后会写入的差异。")).toBeInTheDocument();

    await act(async () => {
      pendingDiff.resolve([]);
    });

    await waitFor(() =>
      expect(within(dialog).getByRole("button", { name: "确认撤回" })).toBeEnabled(),
    );
    expect(within(dialog).getByText("当前受管配置已与将恢复的备份一致。")).toBeInTheDocument();
  });

  it("cancels an undo confirmation without starting the restore transaction", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "备份" }));
    invokeMock.mockClear();
    await user.click(await screen.findByRole("button", { name: "撤回上一次切换" }));
    const dialog = await screen.findByRole("dialog", { name: "撤回上一次切换" });

    await user.click(within(dialog).getByRole("button", { name: "取消" }));

    expect(screen.queryByRole("dialog", { name: "撤回上一次切换" })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("undo_last_switch", expect.anything());
  });

  it("opens the backup folder via the backend command", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "备份" }));
    await user.click(await screen.findByRole("button", { name: "打开备份文件夹" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("open_backup_dir"));
  });

  it("saves general settings without a client-file write and invalidates supplier previews", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "供应商" }));
    await user.click(await screen.findByRole("option", { name: /备用网关/ }));
    await user.click(screen.getByRole("button", { name: "预览 备用网关 变更" }));
    const previewPanel = await screen.findByRole("region", { name: "变更预览" });
    expect(within(previewPanel).getByText("gpt-5.3-codex")).toBeInTheDocument();
    expect(within(previewPanel).getByText("gpt-5.4")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "通用设置" }));
    await user.click(await screen.findByRole("switch", { name: "隐藏推理摘要" }));
    expect(screen.getByText("有未保存修改")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保存通用设置" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("save_common_settings", {
        target: "codex",
        settings: { settings: { hide_agent_reasoning: true } },
        expectedSettingsHash: "codex-settings-hash",
      }),
    );
    expect(invokeMock.mock.calls.map(([command]) => command)).not.toContain("execute_switch");

    // The switch affordance lives on the overview only.
    await user.click(screen.getByRole("button", { name: "概览" }));
    expect(screen.getByRole("button", { name: "安全切换" })).toBeDisabled();
  });

  it("loads, applies, and saves complete application settings", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    expect(await screen.findByRole("radio", { name: "最小化到托盘" })).toBeChecked();
    expect(document.documentElement.dataset.theme).toBeUndefined();
    expect(document.documentElement.dataset.motion).toBeUndefined();

    await user.click(screen.getByRole("radio", { name: "深色" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
        settings: {
          closeBehavior: "hideToTray",
          theme: "dark",
          motion: "system",
          alwaysOnTop: false,
          launchAtLogin: false,
          hardwareAcceleration: true,
          interfaceFont: "Noto Sans SC",
          runtimeLogLevel: "info",
        },
      }),
    );
    expect(document.documentElement.dataset.theme).toBe("dark");

    await user.click(screen.getByRole("switch", { name: "窗口始终置顶" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
        settings: {
          closeBehavior: "hideToTray",
          theme: "dark",
          motion: "system",
          alwaysOnTop: true,
          launchAtLogin: false,
          hardwareAcceleration: true,
          interfaceFont: "Noto Sans SC",
          runtimeLogLevel: "info",
        },
      }),
    );

    await user.click(screen.getByRole("switch", { name: "启用硬件加速" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
        settings: {
          closeBehavior: "hideToTray",
          theme: "dark",
          motion: "system",
          alwaysOnTop: true,
          launchAtLogin: false,
          hardwareAcceleration: false,
          interfaceFont: "Noto Sans SC",
          runtimeLogLevel: "info",
        },
      }),
    );

    await user.click(screen.getByRole("button", { name: "重启应用" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("restart_application"));
  });

  it("顶栏置顶钮经同一保存路径提交完整设置对象", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    const pin = await screen.findByRole("button", { name: "置顶窗口" });
    expect(pin.getAttribute("aria-pressed")).toBe("false");
    await user.click(pin);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
        settings: {
          closeBehavior: "hideToTray",
          theme: "system",
          motion: "system",
          alwaysOnTop: true,
          launchAtLogin: false,
          hardwareAcceleration: true,
          interfaceFont: "Noto Sans SC",
          runtimeLogLevel: "info",
        },
      }),
    );
    expect(await screen.findByRole("button", { name: "取消置顶" })).toBeDefined();
  });

  it("keeps the applied appearance when saving a replacement setting fails", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_app_settings") {
        return Promise.resolve({
          closeBehavior: "hideToTray",
          theme: "dark",
          motion: "reduce",
          alwaysOnTop: false,
          launchAtLogin: false,
          hardwareAcceleration: true,
          interfaceFont: "Noto Sans SC",
          runtimeLogLevel: "info",
        });
      }
      if (command === "set_app_settings") return Promise.reject({ message: "保存失败" });
      if (command === "config_status") return Promise.resolve(statuses);
      if (command === "list_profiles") return Promise.resolve(profiles);
      if (command === "list_backups") return Promise.resolve([]);
      if (command === "lock_status") return Promise.resolve({ state: "free" });
      if (command === "get_common_settings_editor") {
        return Promise.resolve({
          app: "codex",
          settings: { settings: {} },
          settingsHash: "settings-hash",
          groups: [],
          specs: [],
          directory: [],
        });
      }
      return Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    expect(document.documentElement.dataset.motion).toBe("reduce");
    await user.click(screen.getByRole("radio", { name: "浅色" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("保存失败");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.motion).toBe("reduce");
  });

  it("界面字体经完整设置对象保存并立即应用到界面", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const trigger = await screen.findByRole("button", { name: "选择界面字体" });
    expect(trigger.textContent).toContain("Noto Sans SC");
    expect(document.documentElement.style.getPropertyValue("--asb-font-user")).toBe(
      '"Noto Sans SC"',
    );

    await user.click(trigger);
    await user.click(screen.getByRole("option", { name: /Microsoft YaHei/ }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
        settings: {
          closeBehavior: "hideToTray",
          theme: "system",
          motion: "system",
          alwaysOnTop: false,
          launchAtLogin: false,
          hardwareAcceleration: true,
          interfaceFont: "Microsoft YaHei",
          runtimeLogLevel: "info",
        },
      }),
    );
    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("--asb-font-user")).toBe(
        '"Microsoft YaHei"',
      ),
    );
  });

  it("设置加载失败时固定显示原因并支持重试", async () => {
    primeBackend();
    let failSettings = true;
    const backend = invokeMock.getMockImplementation();
    invokeMock.mockImplementation((command: string, args?: Parameters<typeof invoke>[1]) => {
      if (command === "get_app_settings" && failSettings) {
        return Promise.reject({ code: "app-settings-unavailable", message: "应用设置格式无效" });
      }
      return backend?.(command, args) ?? Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    expect(await screen.findByText("设置加载失败：应用设置格式无效")).toBeInTheDocument();
    expect(screen.queryByText("加载中")).toBeNull();
    expect(screen.queryByRole("radiogroup", { name: "界面主题" })).toBeNull();

    failSettings = false;
    await user.click(screen.getByRole("button", { name: "重试" }));
    expect(await screen.findByRole("radiogroup", { name: "界面主题" })).toBeInTheDocument();
  });

  it("marks keyboard focus and clears it on pointer interaction", async () => {
    primeBackend();
    render(<App />);
    await screen.findByRole("button", { name: "概览" });
    const root = document.documentElement;
    expect(root.dataset.focusSource).toBeUndefined();

    // Pointer interaction: no keyboard focus ring on drawn controls.
    fireEvent.pointerDown(window);
    expect(root.dataset.focusSource).toBeUndefined();

    // Keyboard navigation marks the modality the rings key off.
    fireEvent.keyDown(window, { key: "Tab" });
    expect(root.dataset.focusSource).toBe("key");
    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(root.dataset.focusSource).toBe("key");

    // Any pointer down hands the modality back to the pointer.
    fireEvent.pointerDown(window);
    expect(root.dataset.focusSource).toBeUndefined();
  });

  it("shows a local read failure instead of treating it as a missing file", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "config_status") return Promise.resolve(statuses);
      if (command === "list_profiles") return Promise.resolve(profiles);
      if (command === "list_backups") return Promise.resolve([]);
      if (command === "lock_status") return Promise.resolve({ state: "free" });
      if (command === "get_app_settings") return Promise.resolve(defaultSettings);
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

  it("shows scan results as per-client status cards with route facts and in-card import", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "config_status") return Promise.resolve(statuses);
      if (command === "list_profiles") return Promise.resolve(profiles);
      if (command === "list_backups") return Promise.resolve([]);
      if (command === "lock_status") return Promise.resolve({ state: "free" });
      if (command === "get_app_settings") return Promise.resolve(defaultSettings);
      if (command === "discover_local") {
        return Promise.resolve({
          codex: {
            app: "codex",
            path: "C:/Users/test/.codex/config.toml",
            exists: true,
            state: {
              kind: "ok",
              route: statuses[0].route,
              managed: true,
              warnings: ["存在托管键但未识别到供应商名称"],
              importable: false,
            },
          },
          claude: {
            app: "claude",
            path: "C:/Users/test/.claude/settings.json",
            exists: true,
            state: {
              kind: "ok",
              route: {
                ...statuses[1].route,
                routeMode: "custom",
                baseUrl: "https://relay.internal",
              },
              managed: false,
              warnings: ["settings.json 的 env 中存在明文 ANTHROPIC_AUTH_TOKEN"],
              importable: true,
            },
          },
          importProposals: [
            {
              app: "claude",
              draft: {
                app: "claude",
                name: "当前 Claude 配置",
                model: "claude-sonnet-4",
                baseUrl: "https://relay.internal",
                apiKey: "test-api-key",
                modelOptions: null,
              },
              basis: "由当前 Claude 配置的模型与服务地址生成",
            },
          ],
        });
      }
      if (command === "import_discovered_profile") {
        return Promise.resolve(profiles[0]);
      }
      return Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "发现" }));
    await user.click(screen.getByRole("button", { name: "扫描配置" }));

    const codexCard = await screen.findByLabelText("Codex 扫描结果");
    expect(within(codexCard).getByText("配置正常")).toBeInTheDocument();
    expect(within(codexCard).getByText("自定义服务 · gpt-5.3-codex")).toBeInTheDocument();
    expect(within(codexCard).getByText("本机网关")).toBeInTheDocument();
    expect(within(codexCard).getByText("https://gateway.internal/v1")).toBeInTheDocument();
    expect(within(codexCard).getByText("OPENAI_API_KEY")).toBeInTheDocument();
    expect(within(codexCard).getByText("已由本应用管理")).toBeInTheDocument();
    expect(within(codexCard).getByText(/托管键但未识别到供应商名称/)).toBeInTheDocument();
    expect(within(codexCard).queryByRole("button", { name: "导入供应商" })).not.toBeInTheDocument();

    const claudeCard = screen.getByLabelText("Claude 扫描结果");
    expect(within(claudeCard).getByText("自定义服务 · claude-sonnet-4")).toBeInTheDocument();
    expect(within(claudeCard).getByText("未由本应用管理")).toBeInTheDocument();
    expect(within(claudeCard).getByText(/ANTHROPIC_AUTH_TOKEN/)).toBeInTheDocument();
    expect(within(claudeCard).getByText("由当前 Claude 配置的模型与服务地址生成")).toBeInTheDocument();

    await user.click(within(claudeCard).getByRole("button", { name: "导入供应商" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("import_discovered_profile", { target: "claude" }),
    );
  });

  it("scans CC Switch read-only, previews providers, and imports the selection", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_app_settings") return Promise.resolve(defaultSettings);
      if (command === "scan_ccswitch") return Promise.resolve(ccScan);
      if (command === "import_ccswitch_profiles") {
        return Promise.resolve({
          importedCount: 2,
          usageScriptImportedCount: 1,
          skippedExisting: [],
          notImported: [],
        });
      }
      return Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "发现" }));
    await user.click(screen.getByRole("button", { name: "扫描 CC Switch（只读）" }));

    expect(await screen.findByText("中继 A")).toBeInTheDocument();
    expect(screen.getByText(/将导入用量查询脚本/)).toBeInTheDocument();
    expect(screen.getByText(/无法导入：客户端 gemini 超出本应用支持范围/)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("scan_ccswitch");

    // Every importable row is selected, including the credential-free
    // official route.
    expect(screen.getByRole("checkbox", { name: "中继 A" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Codex 官方登录" })).toBeChecked();

    await user.click(screen.getByRole("button", { name: "导入所选 2 项" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("import_ccswitch_profiles", {
        keys: ["claude:id-1", "codex:id-2"],
      }),
    );
    expect(
      await screen.findByText("已导入 2 项 · 已导入用量脚本 1 项"),
    ).toBeInTheDocument();
  });

  it("opens provider editing in a dedicated view and returns via the back affordance", async () => {
    primeBackend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "供应商" }));
    await user.click(await screen.findByRole("option", { name: /备用网关/ }));
    await user.click(screen.getByRole("button", { name: "编辑 备用网关" }));

    // Dedicated view: focused title, back affordance, list hidden.
    expect(screen.getByRole("heading", { name: "编辑供应商" })).toBeInTheDocument();
    expect(screen.queryByRole("tablist", { name: "客户端" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "返回供应商列表" }));
    expect(await screen.findByRole("tablist", { name: "客户端" })).toBeInTheDocument();
  });

  it("offers a confirmed reset only for an unsupported profile store", async () => {
    primeBackend();
    let unsupported = true;
    const backend = invokeMock.getMockImplementation();
    expect(backend).toBeDefined();
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "list_profiles" && unsupported) {
        return Promise.reject({
          code: "profile-store-unsupported",
          message: "供应商存储格式无效或来自已不受支持的旧版本；请重新创建供应商档案",
        });
      }
      if (command === "reset_profile_store") {
        unsupported = false;
        return Promise.resolve(undefined);
      }
      return backend!(command, args as never);
    });
    const user = userEvent.setup();
    render(<App />);

    const alert = await screen.findByRole("alert", { name: "操作错误" });
    expect(alert).toHaveTextContent("供应商存储格式无效");
    const resetTrigger = screen.getByRole("button", { name: "清空旧档案并重新开始" });
    expect(invokeMock).not.toHaveBeenCalledWith("reset_profile_store", expect.anything());

    await user.click(resetTrigger);
    expect(screen.getByRole("dialog", { name: "清空旧供应商档案" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(invokeMock).not.toHaveBeenCalledWith("reset_profile_store", expect.anything());

    await user.click(screen.getByRole("button", { name: "清空旧档案并重新开始" }));
    await user.click(screen.getByRole("button", { name: "清空并重新开始" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("reset_profile_store", { confirmWrite: true }),
    );
    await waitFor(() => expect(screen.queryByRole("button", { name: "清空旧档案并重新开始" })).not.toBeInTheDocument());
  });

  it("does not offer a reset for an unreadable profile store", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_profiles") {
        return Promise.reject({ code: "store-unreadable", message: "供应商存储不可读" });
      }
      if (command === "get_app_settings") return Promise.resolve(defaultSettings);
      return Promise.resolve([]);
    });
    render(<App />);

    await screen.findByRole("alert");
    expect(screen.queryByRole("button", { name: "清空旧档案并重新开始" })).not.toBeInTheDocument();
  });

  it("surfaces a config-status failure as a typed error", async () => {
    primeBackend();
    invokeMock.mockImplementation((command: string) => {
      if (command === "config_status") {
        return Promise.reject({ code: "read-current", message: "无法读取当前配置" });
      }
      if (command === "get_app_settings") return Promise.resolve(defaultSettings);
      return Promise.resolve([]);
    });
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取当前配置");
  });

  it("keeps the latest provider preview when an older in-flight request lands late", async () => {
    primeBackend();
    const twoProfiles: ProviderRecord[] = [
      {
        profile: {
          id: "codex-a",
          app: "codex",
          routeMode: "custom",
          name: "网关甲",
          model: "model-a",
          baseUrl: "https://a.internal/v1",
          apiKey: "KEY_A",
          modelOptions: null,
          websiteUrl: null,
        },
        fileHash: "a-hash",
      },
      {
        profile: {
          id: "codex-b",
          app: "codex",
          routeMode: "custom",
          name: "网关乙",
          model: "model-b",
          baseUrl: "https://b.internal/v1",
          apiKey: "KEY_B",
          modelOptions: null,
          websiteUrl: null,
        },
        fileHash: "b-hash",
      },
    ];
    const previewFor = (id: string, model: string): FilePreview => ({
      contentHash: `hash-${id}`,
      renderedHash: `rendered-${id}`,
      content: `model = "${model}"\n`,
      preview: {
        app: "codex",
        target: "C:/Users/test/.codex/config.toml",
        changes: [{ key: "model", kind: "set", before: "gpt-5.3-codex", after: model }],
        warnings: [],
        backupDir: "C:/backups",
      },
    });
    const pending = new Map<string, ReturnType<typeof deferred<FilePreview>>>();
    const backend = invokeMock.getMockImplementation();
    expect(backend).toBeDefined();
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "list_profiles") return Promise.resolve(twoProfiles);
      if (command === "preview_switch") {
        const profileId = (args as { profileId: string }).profileId;
        let entry = pending.get(profileId);
        if (!entry) {
          entry = deferred<FilePreview>();
          pending.set(profileId, entry);
        }
        return entry.promise;
      }
      return backend!(command, args as never);
    });
    invokeMock.mockClear();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "供应商" }));
    await user.click(await screen.findByRole("button", { name: "预览 网关甲 变更" }));
    await user.click(screen.getByRole("button", { name: "预览 网关乙 变更" }));
    expect(pending.get("codex-a")).toBeDefined();
    expect(pending.get("codex-b")).toBeDefined();
    const switchCalls = invokeMock.mock.calls.filter(([command]) => command === "preview_switch");
    expect(switchCalls).toHaveLength(2);

    await act(async () => {
      pending.get("codex-b")!.resolve(previewFor("codex-b", "model-b"));
    });
    const panel = await screen.findByRole("region", { name: "变更预览" });
    expect(within(panel).getByText("model-b")).toBeInTheDocument();

    // The older request lands last; it belongs to a superseded selection.
    await act(async () => {
      pending.get("codex-a")!.resolve(previewFor("codex-a", "model-a"));
    });
    expect(within(panel).getByText("model-b")).toBeInTheDocument();
    expect(within(panel).queryByText("model-a")).not.toBeInTheDocument();
  });
});

describe("client boundary", () => {
  it("re-exports commands as typed functions only", () => {
    for (const exported of Object.keys(client)) {
      expect(typeof client[exported as keyof typeof client]).toBe("function");
    }
  });
});
