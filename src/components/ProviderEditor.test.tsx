import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderEditor } from "./ProviderEditor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("ProviderEditor", () => {
  it("submits a Codex custom draft with mode, model options, and an environment-variable name", async () => {
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
    await user.type(screen.getByLabelText("环境变量名"), "OPENAI_API_KEY");
    await user.selectOptions(
      screen.getByLabelText("推理强度"),
      "xhigh",
    );
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "codex",
      mode: "custom",
      name: "本机网关",
      model: "gpt-5.3-codex",
      baseUrl: "https://gateway.example/v1",
      envKey: "OPENAI_API_KEY",
      modelOptions: {
        kind: "codex",
        reasoningEffort: "xhigh",
        reasoningSummary: null,
        verbosity: null,
        contextWindow: null,
      },
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

    await user.type(screen.getByLabelText("名称"), "官方 Claude");
    await user.click(screen.getByLabelText("官方登录"));
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "claude",
      mode: "official",
      name: "官方 Claude",
      model: null,
      baseUrl: null,
      envKey: null,
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
    await user.type(screen.getByLabelText("Haiku 档"), "claude-haiku-4");
    await user.clear(screen.getByLabelText("Haiku 档"));
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "claude",
      mode: "custom",
      name: "清除旧映射",
      model: null,
      baseUrl: "https://relay.example",
      envKey: null,
      modelOptions: {
        kind: "claude",
        haikuModel: null,
        sonnetModel: null,
        opusModel: null,
        availableModels: null,
      },
    });
  });

  it("hides the endpoint field in official mode and shows it in custom mode", async () => {
    const user = userEvent.setup();
    render(
      <ProviderEditor
        profile={null}
        initialApp="codex"
        busy={false}
        onSave={() => {}}
        onCancel={() => {}}
      />,
    );

    await user.click(screen.getByLabelText("官方登录"));
    expect(screen.queryByLabelText("服务地址")).toBeNull();
    expect(screen.queryByLabelText("环境变量名")).toBeNull();

    await user.click(screen.getByLabelText("自定义服务"));
    expect(screen.getByLabelText("服务地址")).toBeInTheDocument();
  });

  it("collects the Claude model tiers including the available list", async () => {
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

    await user.type(screen.getByLabelText("名称"), "中继 C");
    await user.type(screen.getByLabelText("服务地址"), "https://relay-c.internal");
    await user.type(screen.getByLabelText("Haiku 档"), "claude-haiku-4");
    fireEvent.change(screen.getByLabelText("可选模型列表（每行一个）"), {
      target: { value: "claude-opus-4\nclaude-sonnet-4" },
    });
    await user.click(screen.getByRole("button", { name: "保存供应商" }));

    expect(onSave).toHaveBeenCalledWith({
      app: "claude",
      mode: "custom",
      name: "中继 C",
      model: null,
      baseUrl: "https://relay-c.internal",
      envKey: null,
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
