import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { WindowControls } from "./WindowControls";
import {
  closeWindow,
  getWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
} from "../api/client";

vi.mock("../api/client", () => ({
  minimizeWindow: vi.fn(() => Promise.resolve()),
  toggleMaximizeWindow: vi.fn(() => Promise.resolve()),
  closeWindow: vi.fn(() => Promise.resolve()),
  getWindowMaximized: vi.fn(),
  onWindowResized: vi.fn(),
}));

const maximizedMock = vi.mocked(getWindowMaximized);
const onWindowResizedMock = vi.mocked(onWindowResized);

describe("WindowControls", () => {
  it("窗口态渲染三钮，最大化钮为单方块图形", async () => {
    maximizedMock.mockResolvedValue(false);
    onWindowResizedMock.mockResolvedValue(() => {});
    const { container } = render(<WindowControls />);
    await screen.findByRole("button", { name: "最大化" });
    expect(container.querySelectorAll(".asb-winbtn")).toHaveLength(3);
    const maximize = container.querySelector('button[aria-label="最大化"]');
    expect(maximize?.querySelector("svg > rect")).not.toBeNull();
    expect(maximize?.querySelector("svg > path")).toBeNull();
  });

  it("最大化态镜像为还原钮：双方块图形且不再有最大化钮", async () => {
    maximizedMock.mockResolvedValue(true);
    onWindowResizedMock.mockResolvedValue(() => {});
    const { container } = render(<WindowControls />);
    await screen.findByRole("button", { name: "还原" });
    const restore = container.querySelector('button[aria-label="还原"]');
    expect(restore?.querySelector("svg > rect")).not.toBeNull();
    expect(restore?.querySelector("svg > path")).not.toBeNull();
    expect(container.querySelector('button[aria-label="最大化"]')).toBeNull();
  });

  it("窗口 resize 事件重新同步最大化状态", async () => {
    let resizeHandler: () => void = () => {};
    maximizedMock.mockResolvedValue(false);
    onWindowResizedMock.mockImplementation((handler) => {
      resizeHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<WindowControls />);
    await screen.findByRole("button", { name: "最大化" });
    maximizedMock.mockResolvedValue(true);
    await act(async () => {
      resizeHandler();
    });
    expect(await screen.findByRole("button", { name: "还原" })).toBeDefined();
  });

  it("点击三钮分别触发对应命令且各只一次", async () => {
    maximizedMock.mockResolvedValue(false);
    onWindowResizedMock.mockResolvedValue(() => {});
    render(<WindowControls />);
    fireEvent.click(await screen.findByRole("button", { name: "最小化" }));
    fireEvent.click(screen.getByRole("button", { name: "最大化" }));
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    await waitFor(() => expect(minimizeWindow).toHaveBeenCalledOnce());
    await waitFor(() => expect(toggleMaximizeWindow).toHaveBeenCalledOnce());
    await waitFor(() => expect(closeWindow).toHaveBeenCalledOnce());
  });

  it("卸载时停止监听窗口 resize", async () => {
    const stop = vi.fn();
    maximizedMock.mockResolvedValue(false);
    onWindowResizedMock.mockResolvedValue(stop);
    const { unmount } = render(<WindowControls />);
    await waitFor(() => expect(onWindowResizedMock).toHaveBeenCalled());
    unmount();
    await waitFor(() => expect(stop).toHaveBeenCalledOnce());
  });
});
