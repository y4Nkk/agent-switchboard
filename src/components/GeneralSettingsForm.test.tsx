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
    default: { boolValue: false },
    options: [],
  },
  {
    key: "model_reasoning_effort",
    label: "推理强度",
    group: "模型行为",
    control: "slider",
    default: { strValue: "minimal" },
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
    default: { strValue: "read-only" },
    options: [
      { value: "read-only", label: "只读" },
      { value: "workspace-write", label: "工作区可写" },
    ],
  },
];

describe("GeneralSettingsForm", () => {
  it("renders only catalog parameters as concrete controls, without patch states", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <GeneralSettingsForm
        specs={specs}
        groups={["模型行为", "安全与审批"]}
        values={{
          hide_agent_reasoning: false,
          model_reasoning_effort: "high",
          sandbox_mode: "read-only",
        }}
        busy={false}
        onChange={onChange}
        onResetGroup={vi.fn()}
      />,
    );

    expect(screen.getByRole("switch", { name: "隐藏推理摘要" })).not.toBeChecked();
    expect(screen.getByRole("slider", { name: "推理强度" })).toHaveValue("1");
    expect(screen.getByRole("radio", { name: "只读" })).toBeChecked();
    expect(screen.queryByRole("radio", { name: "不接管" })).not.toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: "写入值" })).not.toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: "移除该项" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("switch", { name: "隐藏推理摘要" }));
    expect(onChange).toHaveBeenCalledWith("hide_agent_reasoning", true);
  });

  it("changes a catalog choice immediately and restores a group default through its caller", () => {
    const onChange = vi.fn();
    const onResetGroup = vi.fn();
    render(
      <GeneralSettingsForm
        specs={specs}
        groups={["模型行为", "安全与审批"]}
        values={{
          hide_agent_reasoning: true,
          model_reasoning_effort: "minimal",
          sandbox_mode: "read-only",
        }}
        busy={false}
        onChange={onChange}
        onResetGroup={onResetGroup}
      />,
    );

    fireEvent.change(screen.getByRole("slider", { name: "推理强度" }), {
      target: { value: "2" },
    });
    expect(onChange).toHaveBeenCalledWith("model_reasoning_effort", "xhigh");

    const restoreGroup = screen.getAllByRole("button", { name: "恢复默认值" })[0];
    expect(restoreGroup).toHaveClass("asb-btn-secondary");
    fireEvent.click(restoreGroup);
    expect(onResetGroup).toHaveBeenCalledWith("模型行为");
  });
});
