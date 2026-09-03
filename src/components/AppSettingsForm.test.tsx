import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppSettingsForm } from "./AppSettingsForm";

vi.mock("../api/client", () => ({
  listSystemFonts: vi.fn().mockResolvedValue(["Microsoft YaHei", "等线", "Segoe UI"]),
}));

const settings = {
  closeBehavior: "hideToTray" as const,
  theme: "system" as const,
  motion: "system" as const,
  alwaysOnTop: false,
  launchAtLogin: false,
  hardwareAcceleration: true,
  interfaceFont: "Noto Sans SC",
  runtimeLogLevel: "info" as const,
  collapsedUsageIds: [],
};

function callbacks() {
  return {
    onCloseBehaviorChange: vi.fn(),
    onThemeChange: vi.fn(),
    onMotionChange: vi.fn(),
    onInterfaceFontChange: vi.fn(),
    onAlwaysOnTopChange: vi.fn(),
    onLaunchAtLoginChange: vi.fn(),
    onHardwareAccelerationChange: vi.fn(),
    onRestart: vi.fn(),
  };
}

describe("AppSettingsForm", () => {
  beforeEach(() => {
    // jsdom's default UA reports linux; the suite below asserts the Windows
    // surface including the hardware-acceleration toggle.
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("hides the hardware-acceleration section outside Windows", () => {
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (X11; Linux x86_64)",
    });
    render(<AppSettingsForm settings={settings} busy={false} {...callbacks()} />);

    expect(screen.queryByRole("switch", { name: "启用硬件加速" })).toBeNull();
    expect(screen.queryByRole("radiogroup", { name: "点击关闭按钮时" })).toBeInTheDocument();
  });

  it("renders the persisted close behavior as a selected segment", () => {
    render(<AppSettingsForm settings={settings} busy={false} {...callbacks()} />);

    expect(screen.getByRole("radiogroup", { name: "点击关闭按钮时" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "最小化到托盘" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "退出应用" })).not.toBeChecked();
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
    await user.click(screen.getByRole("switch", { name: "窗口始终置顶" }));
    expect(handlers.onAlwaysOnTopChange).toHaveBeenCalledWith(true);
    await user.click(screen.getByRole("switch", { name: "开机自动启动" }));
    expect(handlers.onLaunchAtLoginChange).toHaveBeenCalledWith(true);
    expect(screen.queryByRole("button", { name: "重启应用" })).toBeNull();
    await user.click(screen.getByRole("switch", { name: "启用硬件加速" }));
    expect(handlers.onHardwareAccelerationChange).toHaveBeenCalledWith(false);
    expect(screen.getByRole("button", { name: "重启应用" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "重启应用" }));
    expect(handlers.onRestart).toHaveBeenCalledTimes(1);
  });

  it("shows the restart action after enabling hardware acceleration", async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(
      <AppSettingsForm
        settings={{ ...settings, hardwareAcceleration: false }}
        busy={false}
        {...handlers}
      />,
    );

    expect(screen.queryByRole("button", { name: "重启应用" })).toBeNull();
    await user.click(screen.getByRole("switch", { name: "启用硬件加速" }));

    expect(handlers.onHardwareAccelerationChange).toHaveBeenCalledWith(true);
    expect(screen.getByRole("button", { name: "重启应用" })).toBeInTheDocument();
  });

  it("renders the interface-font row and emits the chosen font", async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<AppSettingsForm settings={settings} busy={false} {...handlers} />);

    const trigger = screen.getByRole("button", { name: "选择界面字体" });
    expect(trigger.textContent).toContain("Noto Sans SC");
    await user.click(trigger);
    await user.click(screen.getByRole("option", { name: /Microsoft YaHei/ }));
    expect(handlers.onInterfaceFontChange).toHaveBeenCalledWith("Microsoft YaHei");
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
    expect(screen.getByRole("switch", { name: "启用硬件加速" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "重启应用" })).toBeNull();
    expect(screen.getByRole("button", { name: "选择界面字体" })).toBeDisabled();
  });
});
