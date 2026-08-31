import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppSettingsForm } from "./AppSettingsForm";

const settings = {
  closeBehavior: "hideToTray" as const,
  theme: "system" as const,
  motion: "system" as const,
  alwaysOnTop: false,
  hardwareAcceleration: true,
};

function callbacks() {
  return {
    onCloseBehaviorChange: vi.fn(),
    onThemeChange: vi.fn(),
    onMotionChange: vi.fn(),
    onAlwaysOnTopChange: vi.fn(),
    onHardwareAccelerationChange: vi.fn(),
  };
}

describe("AppSettingsForm", () => {
  it("renders the persisted close behavior as a selected segment", () => {
    render(<AppSettingsForm settings={settings} busy={false} {...callbacks()} />);

    expect(screen.getByRole("radiogroup", { name: "点击关闭按钮时" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "最小化到托盘" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "退出应用" })).not.toBeChecked();
    expect(screen.getByText("关闭窗口后应用继续在系统托盘运行")).toBeInTheDocument();
  });

  it("emits the selected close behavior", async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<AppSettingsForm settings={settings} busy={false} {...handlers} />);

    await user.click(screen.getByRole("radio", { name: "退出应用" }));
    expect(handlers.onCloseBehaviorChange).toHaveBeenCalledWith("exit");
  });

  it("renders and emits appearance, window, and hardware acceleration preferences", async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<AppSettingsForm settings={settings} busy={false} {...handlers} />);

    expect(
      within(screen.getByRole("radiogroup", { name: "界面主题" })).getByRole("radio", {
        name: "跟随系统",
      }),
    ).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "深色" }));
    expect(handlers.onThemeChange).toHaveBeenCalledWith("dark");
    await user.click(screen.getAllByRole("radio", { name: "减少动态效果" })[0]);
    expect(handlers.onMotionChange).toHaveBeenCalledWith("reduce");
    await user.click(screen.getByRole("checkbox", { name: "窗口始终置顶" }));
    expect(handlers.onAlwaysOnTopChange).toHaveBeenCalledWith(true);
    expect(screen.getByText("使用 GPU 渲染界面；完整重启应用后生效")).toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "启用硬件加速" }));
    expect(handlers.onHardwareAccelerationChange).toHaveBeenCalledWith(false);
  });

  it("disables close behavior choices while the update is being saved", () => {
    render(
      <AppSettingsForm
        settings={{ ...settings, closeBehavior: "exit" }}
        busy
        {...callbacks()}
      />,
    );

    expect(screen.getByRole("radio", { name: "最小化到托盘" })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "退出应用" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "启用硬件加速" })).toBeDisabled();
  });
});
