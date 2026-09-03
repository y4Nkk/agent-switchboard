import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { CommonSettingSpec } from "../api/client";
import { GeneralSettingsForm } from "./GeneralSettingsForm";

const specs: CommonSettingSpec[] = [
  {
    key: "hide_agent_reasoning",
    label: "隐藏推理摘要",
    group: "模型行为",
    control: "toggle",
    options: [],
  },
  {
    key: "model_reasoning_effort",
    label: "推理强度",
    group: "模型行为",
    control: "slider",
    options: [
      { value: "minimal", label: "极低" },
      { value: "high", label: "高" },
      { value: "xhigh", label: "极高" },
    ],
  },
  {
    key: "sandbox_mode",
    label: "沙箱模式",
    group: "安全与审批",
    control: "segment",
    options: [
      { value: "read-only", label: "只读" },
      { value: "workspace-write", label: "工作区可写" },
    ],
  },
];

describe("GeneralSettingsForm", () => {
  it("renders automatic and explicit values as the only parameter intents", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <GeneralSettingsForm
        specs={specs}
        groups={["模型行为", "安全与审批"]}
        values={{
          hide_agent_reasoning: { mode: "automatic" },
          model_reasoning_effort: { mode: "explicit", value: "high" },
          sandbox_mode: { mode: "explicit", value: "read-only" },
        }}
        busy={false}
        onChange={onChange}
        onResetGroup={vi.fn()}
      />,
    );

    expect(screen.getAllByRole("radio", { name: "自动" })[0]).toBeChecked();
    expect(screen.getByRole("slider", { name: "推理强度" })).toHaveValue("2");
    expect(screen.getByText("当前推理：高")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "只读" })).toBeChecked();
    await user.click(screen.getAllByRole("radio", { name: "开启" })[0]);
    expect(onChange).toHaveBeenCalledWith("hide_agent_reasoning", {
      mode: "explicit",
      value: true,
    });
  });

  it("changes a catalog choice immediately and restores a group default through its caller", () => {
    const onChange = vi.fn();
    const onResetGroup = vi.fn();
    render(
      <GeneralSettingsForm
        specs={specs}
        groups={["模型行为", "安全与审批"]}
        values={{
          hide_agent_reasoning: { mode: "explicit", value: true },
          model_reasoning_effort: { mode: "explicit", value: "minimal" },
          sandbox_mode: { mode: "explicit", value: "read-only" },
        }}
        busy={false}
        onChange={onChange}
        onResetGroup={onResetGroup}
      />,
    );

    fireEvent.change(screen.getByRole("slider", { name: "推理强度" }), {
      target: { value: "3" },
    });
    expect(onChange).toHaveBeenCalledWith("model_reasoning_effort", {
      mode: "explicit",
      value: "xhigh",
    });

    const restoreGroup = screen.getAllByRole("button", { name: "恢复默认值" })[0];
    expect(restoreGroup).toHaveClass("asb-btn-secondary");
    fireEvent.click(restoreGroup);
    expect(onResetGroup).toHaveBeenCalledWith("模型行为");
  });
});
