import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { CodexOfficialQuota, UsageHistorySeries } from "../api/client";
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

const officialHistory: UsageHistorySeries[] = [
  {
    id: "official-7-days",
    label: "7 天",
    unit: "%",
    metric: "usedPercent",
    points: [
      { at: "2026-08-31T03:00:00Z", value: 68 },
      { at: "2026-09-01T03:00:00Z", value: 76.25 },
    ],
  },
];

function callsFor(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command);
}

function mockResetCommands({
  cached,
  refreshed,
}: {
  cached: CodexOfficialQuota | null;
  refreshed?: CodexOfficialQuota | Error;
}) {
  invokeMock.mockImplementation((command) => {
    if (command === "get_cached_codex_official_reset") return Promise.resolve(cached) as never;
    if (command === "get_usage_history") return Promise.resolve(officialHistory) as never;
    if (command === "refresh_codex_official_reset") {
      if (refreshed instanceof Error) return Promise.reject(refreshed) as never;
      if (refreshed === undefined) {
        return Promise.reject(new Error("unexpected official quota refresh")) as never;
      }
      return Promise.resolve(refreshed) as never;
    }
    return Promise.reject(new Error(`unexpected command: ${command}`)) as never;
  });
}

describe("CodexOfficialResetPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("reads the persisted baseline on mount without contacting the network", async () => {
    mockResetCommands({ cached: null });
    render(<CodexOfficialResetPanel />);

    expect(
      await screen.findByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。"),
    ).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_cached_codex_official_reset");
    expect(callsFor("refresh_codex_official_reset")).toHaveLength(0);
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(1));
  });

  it("renders cached windows with countdowns and the detected reset", async () => {
    mockResetCommands({ cached: read });
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
    expect(await screen.findByRole("img", { name: /7 天，76.25 %/ })).toBeInTheDocument();
    expect(callsFor("get_usage_history")).toHaveLength(1);
  });

  it("refreshes the machine login on demand and marks the read live", async () => {
    const user = userEvent.setup();
    mockResetCommands({
      cached: null,
      refreshed: { ...read, at: "2026-09-02T03:00:00Z" },
    });
    render(<CodexOfficialResetPanel />);
    expect(await screen.findByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新官方额度" }));

    expect(await screen.findByText("刚刚刷新")).toBeInTheDocument();
    expect(callsFor("refresh_codex_official_reset")).toHaveLength(1);
    await screen.findByRole("img", { name: /7 天，76.25 %/ });
    expect(callsFor("get_usage_history")).toHaveLength(2);
    expect(screen.queryByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。")).not.toBeInTheDocument();
  });

  it("keeps the last good data and explains a failed refresh", async () => {
    const user = userEvent.setup();
    mockResetCommands({ cached: read, refreshed: new Error("网络不可用") });
    render(<CodexOfficialResetPanel />);
    expect(await screen.findByText("本地缓存")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新官方额度" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "无法刷新官方额度：网络不可用；仍在显示上次成功读取的数据。",
    );
    expect(screen.getByText("76 %")).toBeInTheDocument();
    expect(callsFor("get_usage_history")).toHaveLength(1);
  });

  it("surfaces a recoverable sign-in status without claiming quota data", async () => {
    const user = userEvent.setup();
    mockResetCommands({
      cached: null,
      refreshed: {
        status: "signInRequired",
        windows: [],
        at: null,
        stale: false,
        lastReset: null,
      },
    });
    render(<CodexOfficialResetPanel />);
    expect(await screen.findByText("尚无官方额度读取记录。手动刷新以读取本机 Codex 官方登录。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新官方额度" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("未检测到可用的 Codex 官方登录");
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(callsFor("get_usage_history")).toHaveLength(1);
  });
});
