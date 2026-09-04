import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodexOfficialQuotaPanel } from "./CodexOfficialQuotaPanel";
import type { CodexOfficialQuota, UsageHistorySeries } from "../api/client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const officialHistory: UsageHistorySeries[] = [
  {
    id: "official-5-hours",
    label: "5 小时",
    unit: "%",
    metric: "usedPercent",
    points: [
      { at: "2026-09-01T02:00:00Z", value: 8 },
      { at: "2026-09-01T03:00:00Z", value: 12.5 },
    ],
  },
];

function callsFor(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command);
}

function mockQuotaCommands(quota: CodexOfficialQuota, history = officialHistory) {
  invokeMock.mockImplementation((command) => {
    if (command === "get_usage_history") return Promise.resolve(history) as never;
    if (command === "query_codex_official_quota") return Promise.resolve(quota) as never;
    return Promise.reject(new Error(`unexpected command: ${command}`)) as never;
  });
}

describe("CodexOfficialQuotaPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders the native server windows without receiving OAuth data", async () => {
    const user = userEvent.setup();
    mockQuotaCommands({
      status: "available",
      windows: [
        { label: "5 小时", usedPercent: 12.5, resetsAt: "2026-09-01T08:00:00Z" },
        { label: "7 天", usedPercent: 76.25, resetsAt: "2026-09-06T08:00:00Z" },
      ],
      at: "2026-09-01T03:00:00Z",
      stale: false,
      lastReset: null,
    });

    render(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-codex-official"
        profileId="codex-official"
        profileName="Codex 官方登录"
      />,
    );

    expect(await screen.findByRole("row", { name: /^5 小时/ })).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /^7 天/ })).toBeInTheDocument();
    expect(screen.getAllByText("13 %").length).toBeGreaterThan(0);
    expect(screen.getAllByText("76 %").length).toBeGreaterThan(0);
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    expect(invokeMock).toHaveBeenCalledWith("query_codex_official_quota", {
      profileId: "codex-official",
    });
    expect(callsFor("query_codex_official_quota")[0]?.[1]).toEqual({ profileId: "codex-official" });
    await waitFor(() => expect(document.querySelector(".recharts-line")).not.toBeNull());
    expect(within(screen.getByRole("figure", { name: /官方额度趋势/ })).getAllByText("5 小时").length).toBeGreaterThan(0);
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(2));

    await user.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(callsFor("query_codex_official_quota")).toHaveLength(2));
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(3));
  });

  it("appends a render-time countdown to a declared reset time", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-02T08:00:00Z"));
    mockQuotaCommands({
      status: "available",
      windows: [{ label: "7 天", usedPercent: 76.25, resetsAt: "2026-09-06T08:00:00Z" }],
      at: "2026-09-01T03:00:00Z",
      stale: false,
      lastReset: null,
    });

    try {
      render(
        <CodexOfficialQuotaPanel
          id="codex-official-quota-codex-official"
          profileId="codex-official"
          profileName="Codex 官方登录"
        />,
      );
      await act(async () => {
        await Promise.resolve();
      });

      const windowRow = screen.getByRole("row", { name: /7 天/ });
      expect(windowRow).toHaveTextContent("约 4 天 0 小时后");
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps a stale successful read visible beside the recoverable refresh state", async () => {
    mockQuotaCommands({
      status: "unavailable",
      windows: [{ label: "5 小时", usedPercent: 38, resetsAt: null }],
      at: "2026-09-01T03:00:00Z",
      stale: true,
      lastReset: null,
    });

    render(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-codex-official"
        profileId="codex-official"
        profileName="Codex 官方登录"
      />,
    );

    expect(await screen.findByText("38 %")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("正在显示上次成功读取的额度");
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(1));
  });

  it("directs a missing native login to the Codex login recovery action", async () => {
    mockQuotaCommands({
      status: "signInRequired",
      windows: [],
      at: null,
      stale: false,
      lastReset: null,
    });

    render(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-codex-official"
        profileId="codex-official"
        profileName="Codex 官方登录"
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("完成登录后刷新");
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(1));
  });

  it("re-reads when the profile changes while the first read is in flight", async () => {
    let resolveFirst: (value: CodexOfficialQuota) => void = () => {};
    invokeMock.mockImplementation((command, args) => {
      if (command === "get_usage_history") return Promise.resolve(officialHistory) as never;
      if (command === "query_codex_official_quota") {
        const profileId = (args as { profileId?: string } | undefined)?.profileId;
        return profileId === "codex-official"
          ? new Promise<CodexOfficialQuota>((resolve) => { resolveFirst = resolve; }) as never
          : Promise.resolve({
              status: "available",
              windows: [],
              at: "2026-09-01T03:00:00Z",
              stale: false,
              lastReset: null,
            }) as never;
      }
      return Promise.reject(new Error(`unexpected command: ${command}`)) as never;
    });

    const { rerender } = render(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-codex-official"
        profileId="codex-official"
        profileName="Codex 官方登录"
      />,
    );
    expect(callsFor("query_codex_official_quota")).toHaveLength(1);

    rerender(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-relay"
        profileId="codex-relay"
        profileName="Codex 中继"
      />,
    );

    expect(callsFor("query_codex_official_quota")).toHaveLength(2);
    expect(invokeMock).toHaveBeenLastCalledWith("query_codex_official_quota", {
      profileId: "codex-relay",
    });
    await waitFor(() =>
      expect(screen.queryByText("正在读取官方订阅额度…")).not.toBeInTheDocument(),
    );

    resolveFirst({ status: "available", windows: [], at: null, stale: false, lastReset: null });
    await act(async () => {});
  });
});
