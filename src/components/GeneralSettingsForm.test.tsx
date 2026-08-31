import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GeneralSettingsForm } from "./GeneralSettingsForm";
import type { ChoiceState, ToggleState } from "../api/client";

const toggles: ToggleState[] = [
  {
    key: "disable_response_storage",
    label: "勾选后 OpenAI 服务端不保存你的请求与响应",
    line: "disable_response_storage = true",
    applied: true,
    value: true,
    group: "隐私与数据",
  },
  {
    key: "hide_agent_reasoning",
    label: "在界面中隐藏推理摘要",
    line: "hide_agent_reasoning = true",
    applied: true,
    value: false,
    group: "模型行为",
  },
];

const effort: ChoiceState = {
  key: "model_reasoning_effort",
  label: "推理强度",
  group: "模型行为",
  control: "slider",
  options: [
    { value: "minimal", label: "极低" },
    { value: "low", label: "低" },
    { value: "medium", label: "中" },
    { value: "high", label: "高" },
    { value: "xhigh", label: "极高" },
  ],
  value: "high",
};

const sandbox: ChoiceState = {
  key: "sandbox_mode",
  label: "沙箱模式",
  group: "安全与审批",
  control: "segment",
  options: [
    { value: "read-only", label: "只读" },
    { value: "workspace-write", label: "工作区可写" },
    { value: "danger-full-access", label: "完全访问" },
  ],
  value: null,
};

const groups = ["模型行为", "安全与审批", "隐私与数据"];

describe("GeneralSettingsForm", () => {
  it("renders one checkbox with its real config line per official toggle", () => {
    render(
      <GeneralSettingsForm
        app="codex"
        toggles={toggles}
        choices={[effort, sandbox]}
        groups={groups}
        busy={false}
        onToggle={() => {}}
        onChoiceChange={() => {}}
      />,
    );

    expect(screen.getByRole("checkbox", { name: "勾选后 OpenAI 服务端不保存你的请求与响应" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "在界面中隐藏推理摘要" })).not.toBeChecked();
    expect(screen.getByText("disable_response_storage = true")).toBeInTheDocument();
    expect(screen.getByText("hide_agent_reasoning = true")).toBeInTheDocument();
  });

  it("emits the toggled row, not configuration text", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    render(
      <GeneralSettingsForm
        app="codex"
        toggles={toggles}
        choices={[effort, sandbox]}
        groups={groups}
        busy={false}
        onToggle={onToggle}
        onChoiceChange={() => {}}
      />,
    );

    await user.click(screen.getByRole("checkbox", { name: "在界面中隐藏推理摘要" }));
    expect(onToggle).toHaveBeenCalledWith(toggles[1], true);
  });

  it("shows the live reasoning-effort detent with its config line", () => {
    render(
      <GeneralSettingsForm
        app="codex"
        toggles={[]}
        choices={[effort]}
        groups={groups}
        busy={false}
        onToggle={() => {}}
        onChoiceChange={() => {}}
      />,
    );

    expect(screen.getByRole("slider", { name: "推理强度" })).toHaveValue("4");
    expect(screen.getByText("高")).toBeInTheDocument();
    expect(screen.getByText('model_reasoning_effort = "high"')).toBeInTheDocument();
  });

  it("emits null at the default detent and a detent value elsewhere", () => {
    const onChoiceChange = vi.fn();
    const { rerender } = render(
      <GeneralSettingsForm
        app="codex"
        toggles={[]}
        choices={[effort]}
        groups={groups}
        busy={false}
        onToggle={() => {}}
        onChoiceChange={onChoiceChange}
      />,
    );

    fireEvent.change(screen.getByRole("slider", { name: "推理强度" }), { target: { value: "0" } });
    expect(onChoiceChange).toHaveBeenLastCalledWith(expect.objectContaining({ key: "model_reasoning_effort" }), null);
    fireEvent.change(screen.getByRole("slider", { name: "推理强度" }), { target: { value: "5" } });
    expect(onChoiceChange).toHaveBeenLastCalledWith(expect.objectContaining({ key: "model_reasoning_effort" }), "xhigh");

    rerender(
      <GeneralSettingsForm
        app="codex"
        toggles={[]}
        choices={[{ ...effort, value: null }]}
        groups={groups}
        busy={false}
        onToggle={() => {}}
        onChoiceChange={onChoiceChange}
      />,
    );
    expect(screen.getByText("默认")).toBeInTheDocument();
    expect(screen.queryByText(/model_reasoning_effort/)).not.toBeInTheDocument();
  });

  it("renders grouped sections and emits segment selections", async () => {
    const user = userEvent.setup();
    const onChoiceChange = vi.fn();
    const { rerender } = render(
      <GeneralSettingsForm
        app="codex"
        toggles={toggles}
        choices={[effort, sandbox]}
        groups={groups}
        busy={false}
        onToggle={() => {}}
        onChoiceChange={onChoiceChange}
      />,
    );

    // Section headers appear once per declared group that has members.
    expect(screen.getByRole("heading", { name: "模型行为" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "安全与审批" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "隐私与数据" })).toBeInTheDocument();

    const group = screen.getByRole("radiogroup", { name: "沙箱模式" });
    expect(group).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "工作区可写" }));
    expect(onChoiceChange).toHaveBeenCalledWith(
      expect.objectContaining({ key: "sandbox_mode" }),
      "workspace-write",
    );

    // The applied value flows back through the file state, not local state.
    rerender(
      <GeneralSettingsForm
        app="codex"
        toggles={toggles}
        choices={[effort, { ...sandbox, value: "workspace-write" }]}
        groups={groups}
        busy={false}
        onToggle={() => {}}
        onChoiceChange={onChoiceChange}
      />,
    );
    await user.click(screen.getByRole("radio", { name: "默认" }));
    expect(onChoiceChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ key: "sandbox_mode" }),
      null,
    );
  });
});
