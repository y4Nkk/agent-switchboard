import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { CodexOfficialQuota } from "../api/client";
import { CodexOfficialResetPanel } from "./CodexOfficialResetPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const read: CodexOfficialQuota = {
  status: "available",
  windows: [
    { label: "5 小时", usedPercent: 12.5, resetsAt: "2026-09-02T13:00:00Z" },
    { label: "7 天", usedPercent: 76.25, resetsAt: "2026-09-06T08:00:00Z" },
  ],
  at: "2026-09-01T03:00:00Z",
  stale: false,
  lastReset: {
    observedAt: "2026-08-31T02:34:27Z",
    kind: "scheduled",
    resetsAt: "2026-09-06T08:00:00Z",
  },
};

describe("CodexOfficialResetPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("reads the persisted baseline on mount without contacting the network", async () => {
    invokeMock.mockResolvedValueOnce(null);
    render(<CodexOfficialResetPanel />);

    expect(
      await screen.findByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。"),
    ).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("get_cached_codex_official_reset");
  });

  it("renders cached windows with countdowns and the detected reset", async () => {
    invokeMock.mockResolvedValueOnce(read);
    render(<CodexOfficialResetPanel />);

    expect(await screen.findByRole("row", { name: /7 天/ })).toBeInTheDocument();
    expect(screen.getByText("76 %")).toBeInTheDocument();
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    expect(screen.getByText("本地缓存")).toBeInTheDocument();
    const metas = screen.getAllByText(
      (_, element) => element?.classList.contains("asb-official-reset-meta") === true,
    );
    expect(metas[0]).toHaveTextContent("上次检测到重置：例行重置");
    expect(metas[0]).toHaveTextContent("新重置时间");
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("refreshes the machine login on demand and marks the read live", async () => {
    const user = userEvent.setup();
    invokeMock.mockResolvedValueOnce(null).mockResolvedValueOnce({
      ...read,
      at: "2026-09-02T03:00:00Z",
    });
    render(<CodexOfficialResetPanel />);
    expect(await screen.findByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新官方额度" }));

    expect(await screen.findByText("刚刚刷新")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenNthCalledWith(2, "refresh_codex_official_reset");
    expect(screen.queryByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。")).not.toBeInTheDocument();
  });

  it("keeps the last good data and explains a failed refresh", async () => {
    const user = userEvent.setup();
    invokeMock
      .mockResolvedValueOnce(read)
      .mockRejectedValueOnce(new Error("网络不可用"));
    render(<CodexOfficialResetPanel />);
    expect(await screen.findByText("本地缓存")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新官方额度" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "无法刷新官方额度：网络不可用；仍在显示上次成功读取的数据。",
    );
    expect(screen.getByText("76 %")).toBeInTheDocument();
  });

  it("surfaces a recoverable sign-in status without claiming quota data", async () => {
    const user = userEvent.setup();
    invokeMock.mockResolvedValueOnce(null).mockResolvedValueOnce({
      status: "signInRequired",
      windows: [],
      at: null,
      stale: false,
      lastReset: null,
    });
    render(<CodexOfficialResetPanel />);
    expect(await screen.findByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新官方额度" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("未检测到可用的 Codex 官方登录");
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });
});
