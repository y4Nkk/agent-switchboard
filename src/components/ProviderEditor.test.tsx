import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { ProviderEditor } from "./ProviderEditor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("ProviderEditor", () => {
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
      modelOptions: null,
    });
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
      modelOptions: {
        kind: "claude",
        haikuModel: null,
        sonnetModel: null,
        opusModel: null,
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
      expect.objectContaining({ model: "gpt-5.3-codex", notes: null, websiteUrl: null }),
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
      modelOptions: {
        kind: "claude",
        haikuModel: "claude-haiku-4",
        sonnetModel: null,
        opusModel: null,
        availableModels: ["claude-opus-4", "claude-sonnet-4"],
      },
    });
  });
});
