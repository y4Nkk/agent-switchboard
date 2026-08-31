import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { ProviderEditor } from "./ProviderEditor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("ProviderEditor", () => {
  it("keeps only connectivity and model fetching beside the main-model field", () => {
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
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

  it("preserves an existing usage query while saving other provider fields", () => {
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={{
          id: "profile-a",
          app: "codex",
          name: "中继 A",
          model: null,
          baseUrl: "https://relay.example/v1",
          apiKey: "sk-test",
          modelOptions: null,
          usageQuery: {
            kind: "declarative",
            url: "{{baseUrl}}/balance",
            remainingPath: "data/balance",
          },
        }}
        initialApp="codex"
        busy={false}
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
          name: "密钥档案",
          model: null,
          baseUrl: "https://gateway.example/v1",
          apiKey: "sk-test-secret",
          modelOptions: null,
        }}
        initialApp="codex"
        busy={false}
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
        }}
        initialApp="claude"
        busy={false}
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

  it("offers no route-mode choice and always emits custom drafts", () => {
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    expect(screen.queryByRole("radiogroup", { name: "路由模式" })).toBeNull();
    expect(screen.queryByLabelText("官方登录")).toBeNull();
    expect(screen.getByLabelText("服务地址")).toBeInTheDocument();
    expect(screen.getByLabelText("API 密钥")).toBeInTheDocument();
  });

  it("converts a legacy endpoint-less profile to a custom draft on save", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={{
          id: "legacy-official",
          app: "claude",
          name: "旧官方 Claude",
          model: null,
          baseUrl: null,
          apiKey: "sk-test-key",
          modelOptions: null,
        }}
        initialApp="claude"
        busy={false}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    expect(screen.getByLabelText("服务地址")).toBeInTheDocument();
    await user.type(screen.getByLabelText("服务地址"), "https://relay.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ baseUrl: "https://relay.example" }),
    );
  });

  it("fetches the model list from the service address and fills the primary model", async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockResolvedValue(["gpt-5.2", "gpt-5.3-codex"]);
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        onSave={onSave}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "本机网关");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example/v1");
    await user.type(screen.getByLabelText("API 密钥"), "sk-test-key");
    await user.click(screen.getByRole("button", { name: "获取模型" }));

    expect(await screen.findByRole("combobox", { name: "选择模型" })).toBeInTheDocument();
    // The invoke keys must match the Rust command signature (`url`, `apiKey`).
    expect(invokeMock).toHaveBeenCalledWith("fetch_provider_models", {
      url: "https://gateway.example/v1",
      apiKey: "sk-test-key",
    });

    await user.click(screen.getByRole("combobox", { name: "选择模型" }));
    await user.click(await screen.findByRole("option", { name: "gpt-5.3-codex" }));
    await user.click(screen.getByRole("button", { name: "保存供应商" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ model: "gpt-5.3-codex", notes: null, websiteUrl: null, usageQuery: null }),
    );
    invokeMock.mockReset();
  });

  it("passes the entered API key to the model-list request", async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockResolvedValue(["gpt-5.2"]);
    const user = userEvent.setup();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        onSave={vi.fn()}
        onCancel={() => {}}
      />,
    );

    await user.type(screen.getByLabelText("名称"), "本机网关");
    await user.type(screen.getByLabelText("服务地址"), "https://gateway.example");
    await user.type(screen.getByLabelText("API 密钥"), "sk-entered-key");
    await user.click(screen.getByRole("button", { name: "获取模型" }));

    expect(await screen.findByRole("combobox", { name: "选择模型" })).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith(
      "fetch_provider_models",
      expect.objectContaining({ url: "https://gateway.example", apiKey: "sk-entered-key" }),
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
