import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { ProviderEditor as ProviderEditorComponent } from "./ProviderEditor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function ProviderEditor({
  userConfigWarnings = [],
  onOpenOfficial = vi.fn(),
  ...props
}: Omit<ComponentProps<typeof ProviderEditorComponent>, "userConfigWarnings" | "onOpenOfficial"> & {
  userConfigWarnings?: string[];
  onOpenOfficial?: (app: "codex" | "claude") => void;
}) {
  return (
    <ProviderEditorComponent
      {...props}
      userConfigWarnings={userConfigWarnings}
      onOpenOfficial={onOpenOfficial}
    />
  );
}

describe("ProviderEditor", () => {
  it("decorates the client field with the selected client's brand mark", async () => {
    const user = userEvent.setup();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    const mark = screen.getByLabelText("客户端")
      .closest(".asb-client-control")
      ?.querySelector("img.asb-edit-logo");
    expect(mark).not.toBeNull();
    // Vite inlines assets as data URIs in tests, so only distinctness is stable.
    const codexSrc = mark?.getAttribute("src") ?? "";
    expect(codexSrc.length).toBeGreaterThan(0);

    await user.click(screen.getByRole("combobox", { name: "客户端" }));
    await user.click(await screen.findByRole("option", { name: "Claude" }));

    expect(screen.getByLabelText("客户端").closest(".asb-client-control")
      ?.querySelector("img.asb-edit-logo")
      ?.getAttribute("src")).not.toBe(codexSrc);
  });

  it("keeps only connectivity and model fetching beside the main-model field", () => {
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    fireEvent.change(screen.getByLabelText("服务地址"), {
      target: { value: "https://relay.example/v1" },
    });
    const connectivityToggle = screen.getByRole("button", { name: "检测连通" });
    expect(connectivityToggle.parentElement).toHaveClass("asb-model-actions");
    expect(screen.getByLabelText("服务地址").parentElement).not.toContainElement(connectivityToggle);
    expect(screen.getByRole("button", { name: "获取模型" }).parentElement).toHaveClass(
      "asb-model-actions",
    );
    expect(screen.queryByRole("button", { name: "查询用量" })).not.toBeInTheDocument();
  });

  it("shows the user-level configuration model and its scope warning while editing", () => {
    const { rerender } = render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel="glm-4.6"
        userConfigWarnings={["使用 --profile 启动时会覆盖这里的用户级设置"]}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    expect(screen.getByText("当前用户级配置模型：glm-4.6")).toBeInTheDocument();
    expect(screen.getByText("使用 --profile 启动时会覆盖这里的用户级设置")).toBeInTheDocument();

    rerender(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );
    expect(screen.queryByText(/当前用户级配置模型/)).toBeNull();
  });

  it("preserves an existing usage query while saving other provider fields", () => {
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={{
          id: "profile-a",
          app: "codex",
          routeMode: "custom",
          name: "中继 A",
          model: null,
          baseUrl: "https://relay.example/v1",
          apiKey: "sk-test",
          modelOptions: null,
          websiteUrl: null,
          usageQuery: {
            kind: "declarative",
            url: "{{baseUrl}}/balance",
            remainingPath: "data/balance",
            refreshIntervalMinutes: 0,
          },
        }}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "中继 A（更新）" } });
    fireEvent.click(screen.getByRole("button", { name: "保存供应商" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        usageQuery: {
          kind: "declarative",
          url: "{{baseUrl}}/balance",
          remainingPath: "data/balance",
          usedPath: null,
          totalPath: null,
          unit: null,
          refreshIntervalMinutes: 0,
        },
      }),
    );
  });

  it("submits a Codex draft with model options and an API key", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "本机网关");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example/v1");
    await user.type(screen.getByLabelText("主模型"), "gpt-5.3-codex");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-codex");
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "codex",
      routeMode: "custom",
      name: "本机网关",
      model: "gpt-5.3-codex",
      baseUrl: "https://gateway.example/v1",
      apiKey: "sk-test-codex",
      notes: null,
      websiteUrl: null,
      usageQuery: null,
      modelOptions: null,
    });
  });

  it("reveals and hides the API key only on explicit action", async () => {
    const user = userEvent.setup();
    render(
      <ProviderEditor
        profile={{
          id: "secret-profile",
          app: "codex",
          routeMode: "custom",
          name: "密钥档案",
          model: null,
          baseUrl: "https://gateway.example/v1",
          apiKey: "sk-test-secret",
          modelOptions: null,
          websiteUrl: null,
        }}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    const apiKey = screen.getByLabelText("API 密钥");
    expect(apiKey).toHaveAttribute("type", "password");
    const reveal = screen.getByRole("button", { name: "查看密钥" });
    expect(reveal).toHaveAttribute("aria-pressed", "false");

    await user.click(reveal);

    expect(apiKey).toHaveAttribute("type", "text");
    expect(apiKey).toHaveValue("sk-test-secret");
    const hide = screen.getByRole("button", { name: "隐藏密钥" });
    expect(hide).toHaveAttribute("aria-pressed", "true");

    await user.click(hide);

    expect(apiKey).toHaveAttribute("type", "password");
    expect(screen.getByRole("button", { name: "查看密钥" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("maps the 1M context checkbox to the fixed context-window value", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    const contextWindow = screen.getByRole("checkbox", { name: "启用 1M 上下文窗口" });
    expect(contextWindow).not.toBeChecked();
    expect(screen.queryByRole("spinbutton")).toBeNull();

    await user.click(contextWindow);
    expect(contextWindow).toBeChecked();
    await user.type(screen.getByLabelText("名称"), "百万上下文网关");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example/v1");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-codex");
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        modelOptions: { kind: "codex", contextWindow: 1_000_000 },
      }),
    );

    onSave.mockClear();
    await user.click(contextWindow);
    expect(contextWindow).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "保存供应商" }));
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ modelOptions: null }));
  });

  it("maps Claude Code 1M checkboxes to explicit semantic model state", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "百万上下文 Claude");
    await user.type(screen.getByLabelText("服务地址"), "https://relay.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-claude");
    await user.type(screen.getByLabelText("主模型"), "claude-opus-4-1");
    await user.type(screen.getByLabelText("Sonnet 档"), "claude-sonnet-4-6");
    await user.type(screen.getByLabelText("Opus 档"), "claude-opus-4-1");

    expect(screen.queryByRole("checkbox", { name: "Haiku 档启用 1M 上下文" })).toBeNull();
    const primaryOneM = screen.getByRole("checkbox", { name: "主模型启用 1M 上下文" });
    const sonnetOneM = screen.getByRole("checkbox", { name: "Sonnet 档启用 1M 上下文" });
    const opusOneM = screen.getByRole("checkbox", { name: "Opus 档启用 1M 上下文" });
    expect(primaryOneM).not.toBeChecked();
    expect(sonnetOneM).not.toBeChecked();
    expect(opusOneM).not.toBeChecked();

    await user.click(primaryOneM);
    await user.click(sonnetOneM);
    await user.click(opusOneM);

    expect(screen.getByLabelText("主模型")).toHaveValue("claude-opus-4-1");
    expect(screen.getByLabelText("Sonnet 档")).toHaveValue("claude-sonnet-4-6");
    expect(screen.getByLabelText("Opus 档")).toHaveValue("claude-opus-4-1");

    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        model: "claude-opus-4-1",
        modelOptions: {
          kind: "claude",
          primaryOneM: true,
          haikuModel: null,
          sonnetModel: "claude-sonnet-4-6",
          sonnetOneM: true,
          opusModel: "claude-opus-4-1",
          opusOneM: true,
          availableModels: null,
        },
      }),
    );
  });

  it("renders saved Claude 1M state through the matching checkboxes", () => {
    render(
      <ProviderEditor
        profile={{
          id: "claude-1m",
          app: "claude",
          routeMode: "custom",
          name: "已有 Claude 1M",
          model: "claude-opus-4-1",
          baseUrl: "https://relay.example",
          apiKey: "sk-test-claude",
          modelOptions: {
            kind: "claude",
            primaryOneM: true,
            haikuModel: "claude-haiku-4",
            sonnetModel: "claude-sonnet-4-6",
            sonnetOneM: true,
            opusModel: "claude-opus-4-1",
            opusOneM: false,
            availableModels: null,
          },
          websiteUrl: null,
        }}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    expect(screen.getByLabelText("主模型")).toHaveValue("claude-opus-4-1");
    expect(screen.getByRole("checkbox", { name: "主模型启用 1M 上下文" })).toBeChecked();
    expect(screen.getByLabelText("Haiku 档")).toHaveValue("claude-haiku-4");
    expect(screen.getByLabelText("Sonnet 档")).toHaveValue("claude-sonnet-4-6");
    expect(screen.getByRole("checkbox", { name: "Sonnet 档启用 1M 上下文" })).toBeChecked();
  });

  it("drops an all-empty model-mapping block instead of storing it", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "中继 D");
    await user.type(screen.getByLabelText("服务地址"), "https://relay-d.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "claude",
      routeMode: "custom",
      name: "中继 D",
      model: null,
      baseUrl: "https://relay-d.example",
      apiKey: "sk-test-key",
      notes: null,
      websiteUrl: null,
      usageQuery: null,
      modelOptions: null,
    });
  });

  it("keeps an explicitly cleared Claude mapping so the next switch removes old tiers", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "清除旧映射");
    await user.type(screen.getByLabelText("服务地址"), "https://relay.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.type(screen.getByLabelText("Haiku 档"), "claude-haiku-4");
    await user.clear(screen.getByLabelText("Haiku 档"));
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "claude",
      routeMode: "custom",
      name: "清除旧映射",
      model: null,
      baseUrl: "https://relay.example",
      apiKey: "sk-test-key",
      notes: null,
      websiteUrl: null,
      usageQuery: null,
      modelOptions: {
        kind: "claude",
        primaryOneM: false,
        haikuModel: null,
        sonnetModel: null,
        sonnetOneM: false,
        opusModel: null,
        opusOneM: false,
        availableModels: null,
      },
    });
  });

  it("offers the access-mode choice only when creating and defaults to custom", () => {
    const onSave = vi.fn();
    const { rerender } = render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    expect(screen.getByRole("radiogroup", { name: "接入方式" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "自定义 API 中继" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "官方登录" })).not.toBeChecked();
    expect(screen.getByLabelText("服务地址")).toBeInTheDocument();
    expect(screen.getByLabelText("API 密钥")).toBeInTheDocument();

    rerender(
      <ProviderEditor
        profile={{
          id: "existing",
          app: "codex",
          routeMode: "custom",
          name: "中继",
          model: null,
          baseUrl: "https://gateway.example/v1",
          apiKey: "sk-test",
          modelOptions: null,
          websiteUrl: null,
        }}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    expect(screen.queryByRole("radiogroup", { name: "接入方式" })).toBeNull();
  });

  it("opens the existing official profile instead of creating a duplicate", async () => {
    const user = userEvent.setup();
    const onOpenOfficial = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={["codex"]}
        userConfigModel={null}
        onOpenOfficial={onOpenOfficial}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    expect(screen.getByRole("radio", { name: "自定义 API 中继" })).toBeEnabled();
    const official = screen.getByRole("radio", { name: "官方登录" });
    expect(official).toBeEnabled();
    await user.click(official);
    expect(onOpenOfficial).toHaveBeenCalledWith("codex");
  });

  it("starts a new empty draft when the selected client changes", async () => {
    const user = userEvent.setup();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "Codex 中继");
    await user.type(screen.getByLabelText("官网地址"), "https://codex.example");
    await user.type(screen.getByLabelText("备注"), "Codex 记录");
    await user.type(screen.getByLabelText("服务地址"), "https://relay.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");

    await user.click(screen.getByRole("combobox", { name: "客户端" }));
    await user.click(await screen.findByRole("option", { name: "Claude" }));
    await user.click(screen.getByRole("radio", { name: "官方登录" }));
    expect(screen.getByLabelText("名称")).toHaveValue("Claude 官方登录");

    await user.click(screen.getByRole("combobox", { name: "客户端" }));
    await user.click(await screen.findByRole("option", { name: "Codex" }));

    expect(screen.getByRole("radio", { name: "自定义 API 中继" })).toBeChecked();
    expect(screen.getByLabelText("名称")).toHaveValue("");
    expect(screen.getByLabelText("官网地址")).toHaveValue("");
    expect(screen.getByLabelText("备注")).toHaveValue("");
    expect(screen.getByLabelText("服务地址")).toHaveValue("");
    expect(screen.getByLabelText("API 密钥")).toHaveValue("");
  });

  it("hides custom fields in official mode and gates saving until the login completes", async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockImplementation((command: string) => {
      if (command === "official_login_start") {
        return Promise.resolve({
          userCode: "CODE-1234",
          verificationUrl: "https://auth.openai.com/codex/device",
        });
      }
      if (command === "official_login_poll") {
        return Promise.resolve({
          phase: "completed",
          userCode: null,
          verificationUrl: "",
          message: null,
        });
      }
      return Promise.resolve([]);
    });
    vi.useFakeTimers();
    const onSave = vi.fn();

    try {
      render(
        <ProviderEditor
          profile={null}
          initialApp="codex"
          busy={false}
          officialTakenApps={[]}
          userConfigModel={null}
          onSave={onSave}
          onCancel={() => {}}
        />,
      );

      fireEvent.click(screen.getByRole("radio", { name: "官方登录" }));

      expect(screen.getByLabelText("名称")).toHaveValue("Codex 官方登录");
      expect(screen.queryByLabelText("服务地址")).toBeNull();
      expect(screen.queryByLabelText("主模型")).toBeNull();
      expect(screen.queryByLabelText("API 密钥")).toBeNull();

      const save = screen.getByRole("button", { name: "保存供应商" });
      expect(save).toBeDisabled();

      fireEvent.click(screen.getByRole("button", { name: "开始官方登录" }));
      await act(async () => {});
      expect(screen.getByText(/验证码/)).toBeInTheDocument();
      expect(save).toBeDisabled();

      await act(async () => {
        vi.advanceTimersByTime(3000);
      });
      expect(screen.getByText("登录完成，登录凭据已写入客户端本地文件。")).toBeInTheDocument();
      expect(save).toBeEnabled();

      fireEvent.click(save);
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          app: "codex",
          routeMode: "official",
          name: "Codex 官方登录",
          model: null,
          baseUrl: null,
          apiKey: "",
          modelOptions: null,
          usageQuery: null,
        }),
      );
    } finally {
      vi.useRealTimers();
      invokeMock.mockReset();
    }
  });

  it("saves an edited official profile without demanding a fresh login", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={{
          id: "codex-official",
          app: "codex",
          routeMode: "official",
          name: "Codex 官方登录",
          model: null,
          baseUrl: null,
          apiKey: "",
          modelOptions: null,
          websiteUrl: null,
        }}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    expect(screen.queryByLabelText("服务地址")).toBeNull();
    expect(screen.getByRole("button", { name: "开始官方登录" })).toBeInTheDocument();

    const save = screen.getByRole("button", { name: "保存供应商" });
    expect(save).toBeEnabled();

    await user.clear(screen.getByLabelText("名称"));
    await user.type(screen.getByLabelText("名称"), "Codex 官方");
    await user.click(save);

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ routeMode: "official", name: "Codex 官方" }),
    );
  });

  it("clears the custom-route fields the moment official mode is chosen", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "我的中继");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example/v1");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("radio", { name: "官方登录" }));

    expect(screen.getByLabelText("名称")).toHaveValue("我的中继");
    expect(screen.queryByLabelText("服务地址")).toBeNull();

    await user.click(screen.getByRole("radio", { name: "自定义 API 中继" }));

    expect(screen.getByLabelText("名称")).toHaveValue("我的中继");
    expect(screen.getByLabelText("服务地址")).toHaveValue("");
    expect(screen.getByLabelText("API 密钥")).toHaveValue("");
  });

  it("keeps an existing custom profile in the custom draft contract", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={{
          id: "legacy-official",
          app: "claude",
          routeMode: "custom",
          name: "Claude 中继",
          model: null,
          baseUrl: "https://relay.example",
          apiKey: "sk-test-key",
          modelOptions: null,
          websiteUrl: null,
        }}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    expect(screen.getByLabelText("服务地址")).toHaveValue("https://relay.example");
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ baseUrl: "https://relay.example" }),
    );
  });

  it("fetches the model list from the service address and fills the primary model", async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockResolvedValue([
      { id: "gpt-5.2", ownedBy: "openai" },
      { id: "gpt-5.3-codex", ownedBy: null },
    ]);
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "本机网关");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example/v1");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("button", { name: "获取模型" }));

    const picker = await screen.findByRole("button", { name: "选择模型" });
    // The invoke keys must match the Rust command signature (`url`, `apiKey`).
    expect(invokeMock).toHaveBeenCalledWith("fetch_provider_models", {
      url: "https://gateway.example/v1",
      apiKey: "sk-test-key",
    });

    await user.click(picker);
    await user.click(await screen.findByRole("option", { name: "gpt-5.3-codex" }));
    await user.click(screen.getByRole("button", { name: "保存供应商" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ model: "gpt-5.3-codex", notes: null, websiteUrl: null, usageQuery: null }),
    );
    invokeMock.mockReset();
  });

  it("passes the entered API key to the model-list request", async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockResolvedValue([{ id: "gpt-5.2", ownedBy: "openai" }]);
    const user = userEvent.setup();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "本机网关");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-entered-key");
    await user.click(screen.getByRole("button", { name: "获取模型" }));

    expect(await screen.findByRole("button", { name: "选择模型" })).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith(
      "fetch_provider_models",
      expect.objectContaining({ url: "https://gateway.example", apiKey: "sk-entered-key" }),
    );
    invokeMock.mockReset();
  });

  it("fills the Claude mapping tiers from the same fetched list", async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockResolvedValue([
      { id: "claude-haiku-4-5", ownedBy: "anthropic" },
      { id: "deepseek-v4", ownedBy: "deepseek" },
    ]);
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "中继 B");
    await user.type(screen.getByLabelText("服务地址"), "https://relay.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("button", { name: "获取模型" }));

    await user.click(await screen.findByRole("button", { name: "选择 Haiku 档模型" }));
    await user.click(await screen.findByRole("option", { name: "claude-haiku-4-5" }));
    await user.click(screen.getByRole("button", { name: "选择 Sonnet 档模型" }));
    await user.click(await screen.findByRole("option", { name: "deepseek-v4" }));

    await user.click(screen.getByRole("button", { name: "保存供应商" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        modelOptions: expect.objectContaining({
          kind: "claude",
          haikuModel: "claude-haiku-4-5",
          sonnetModel: "deepseek-v4",
        }),
      }),
    );
    invokeMock.mockReset();
  });

  it("keeps metadata fields optional and submits them when filled", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "中继 E");
    await user.type(screen.getByLabelText("官网地址"), "https://provider.example");
    await user.type(screen.getByLabelText("备注"), "团队共用");
    await user.type(screen.getByLabelText("服务地址"), "https://relay-e.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ notes: "团队共用", websiteUrl: "https://provider.example" }),
    );
  });

  it("collects the Claude model tiers including the available list", async () => {    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="claude"
        busy={false}
        officialTakenApps={[]}
        userConfigModel={null}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "中继 C");
    await user.type(screen.getByLabelText("服务地址"), "https://relay-c.internal");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.type(screen.getByLabelText("Haiku 档"), "claude-haiku-4");
    fireEvent.change(screen.getByLabelText("可选模型列表（每行一个）"), {
      target: { value: "claude-opus-4\nclaude-sonnet-4" },
    });
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "claude",
      routeMode: "custom",
      name: "中继 C",
      model: null,
      baseUrl: "https://relay-c.internal",
      apiKey: "sk-test-key",
      notes: null,
      websiteUrl: null,
      usageQuery: null,
      modelOptions: {
        kind: "claude",
        primaryOneM: false,
        haikuModel: "claude-haiku-4",
        sonnetModel: null,
        sonnetOneM: false,
        opusModel: null,
        opusOneM: false,
        availableModels: ["claude-opus-4", "claude-sonnet-4"],
      },
    });
  });
});
