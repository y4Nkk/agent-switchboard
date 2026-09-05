import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TrayPanel } from "./TrayPanel";
import { getTraySnapshot, hideTray, onTrayChanged, openTrayMain, resizeTray, switchTrayProvider, trayReady, type TraySnapshot } from "../api/client";

vi.mock("../api/client", () => ({
  getTraySnapshot: vi.fn(), onTrayChanged: vi.fn(), trayReady: vi.fn(), hideTray: vi.fn(),
  openTrayMain: vi.fn(), quitTray: vi.fn(), resizeTray: vi.fn(), switchTrayProvider: vi.fn(),
}));

const snapshot: TraySnapshot = {
  settings: null, error: null, switching: false,
  providers: [
    { id: "first", app: "codex", name: "当前档案", model: "model-one", active: true, usage: { at: "2026-09-05T00:00:00Z", readings: [{ remaining: 30, used: 10, total: 40, unit: "CNY" }] } },
    { id: "second", app: "codex", name: "第二档案", model: "model-two", active: false, usage: null },
  ],
};
let changed: () => void;
const unlisten = vi.fn();
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(300);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(200);
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(200);
  vi.mocked(getTraySnapshot).mockResolvedValue(snapshot);
  vi.mocked(onTrayChanged).mockImplementation(async (callback) => { changed = callback; return unlisten; });
  vi.mocked(trayReady).mockResolvedValue(undefined);
  vi.mocked(hideTray).mockResolvedValue(undefined);
  vi.mocked(openTrayMain).mockResolvedValue(undefined);
  vi.mocked(resizeTray).mockResolvedValue(undefined);
  vi.mocked(switchTrayProvider).mockResolvedValue(undefined);
});

describe("TrayPanel", () => {
  it("renders active providers, shared quota summary and cache time without querying usage", async () => {
    render(<TrayPanel />);
    expect(await screen.findByRole("button", { name: "当前档案，当前供应商" })).toBeDisabled();
    expect(screen.getByText("剩余 75% · 余额 30 CNY")).toBeInTheDocument();
    expect(screen.getByText("2026年09月05日 08：00")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "当前档案，当前供应商" })).toHaveAccessibleDescription(/model-one.*剩余 75%.*缓存/);
    expect(screen.getByText("暂无供应商")).toBeInTheDocument();
    await waitFor(() => expect(trayReady).toHaveBeenCalledOnce());
  });

  it("preserves providers and quota when switching fails, allowing retry", async () => {
    vi.mocked(switchTrayProvider).mockRejectedValueOnce("配置已变化，请重新预览");
    const user = userEvent.setup();
    render(<TrayPanel />);
    const button = await screen.findByRole("button", { name: "切换到 第二档案" });
    await user.click(button);
    expect(await screen.findByRole("alert")).toHaveTextContent("配置已变化，请重新预览");
    expect(screen.getByText("剩余 75% · 余额 30 CNY")).toBeInTheDocument();
    await user.click(button);
    expect(switchTrayProvider).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("ignores stale snapshots and removes the event subscription", async () => {
    const { unmount } = render(<TrayPanel />);
    await screen.findByText("model-one");
    let resolveOld!: (value: TraySnapshot) => void;
    vi.mocked(getTraySnapshot).mockImplementationOnce(() => new Promise((resolve) => { resolveOld = resolve; }));
    act(() => changed());
    vi.mocked(getTraySnapshot).mockResolvedValueOnce({ ...snapshot, providers: [] });
    await act(async () => changed());
    await act(async () => resolveOld(snapshot));
    expect(screen.queryByText("model-one")).not.toBeInTheDocument();
    unmount();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("keeps recovery actions available and signals readiness after a read failure", async () => {
    vi.mocked(getTraySnapshot).mockRejectedValue("档案文件不可读");
    const user = userEvent.setup();
    render(<TrayPanel />);
    expect(await screen.findByRole("alert")).toHaveTextContent("档案文件不可读");
    await waitFor(() => expect(trayReady).toHaveBeenCalledOnce());
    await user.click(screen.getByRole("button", { name: "管理供应商" }));
    expect(openTrayMain).toHaveBeenCalledWith(true);
    await user.keyboard("{Escape}");
    expect(hideTray).toHaveBeenCalledOnce();
  });

  it("waits for committed layout sizing before signaling readiness, once under StrictMode", async () => {
    const finishes: Array<() => void> = [];
    vi.mocked(resizeTray).mockImplementation(() => new Promise((resolve) => finishes.push(resolve)));
    render(<StrictMode><TrayPanel /></StrictMode>);
    await screen.findByText("model-one");
    expect(trayReady).not.toHaveBeenCalled();
    await act(async () => { finishes.forEach((finish) => finish()); });
    await waitFor(() => expect(trayReady).toHaveBeenCalledOnce());
  });

  it("does not signal readiness after unmounting with a resize in flight", async () => {
    const finishes: Array<() => void> = [];
    vi.mocked(resizeTray).mockImplementation(() => new Promise((resolve) => finishes.push(resolve)));
    const { unmount } = render(<TrayPanel />);
    await screen.findByText("model-one");
    unmount();
    await act(async () => { finishes.forEach((finish) => finish()); });
    expect(trayReady).not.toHaveBeenCalled();
  });

  it("reveals recovery actions when sizing fails instead of waiting forever", async () => {
    vi.mocked(resizeTray).mockRejectedValue("无法调整托盘窗口尺寸");
    render(<TrayPanel />);
    expect(await screen.findByRole("alert")).toHaveTextContent("无法调整托盘窗口尺寸");
    await waitFor(() => expect(trayReady).toHaveBeenCalledOnce());
    expect(screen.getByRole("button", { name: "打开主界面" })).toBeEnabled();
  });
});
