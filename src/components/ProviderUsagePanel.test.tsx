import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderUsagePanel } from "./ProviderUsagePanel";
import type { UsageHistorySeries, UsageSummary } from "../api/client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const usageQuery = {
  kind: "declarative" as const,
  url: "{{baseUrl}}/user/balance",
  remainingPath: "data/balance",
  usedPath: null,
  totalPath: null,
  unit: "CNY",
  refreshIntervalMinutes: 0,
};

const profile = {
  id: "relay-a",
  app: "codex" as const,
  routeMode: "custom" as const,
  name: "中继 A",
  model: null,
  baseUrl: "https://relay.example/v1",
  apiKey: "test-key",
  modelOptions: null,
  websiteUrl: null,
  usageQuery,
};

const successfulSummary: UsageSummary = {
  readings: [
    { planName: "主套餐", remaining: 1024.5, used: 24, total: 1048.5, unit: "CNY" },
    { planName: "附加套餐", remaining: 8, used: null, total: 10, unit: "USD" },
  ],
  at: "2026-08-31T08:00:00Z",
};

const providerHistory: UsageHistorySeries[] = [
  {
    id: "relay-a-main-remaining",
    label: "主套餐",
    unit: "CNY",
    metric: "remaining",
    points: [
      { at: "2026-08-30T08:00:00Z", value: 1060.5 },
      { at: "2026-08-31T08:00:00Z", value: 1024.5 },
    ],
  },
];

const mixedMetricHistory: UsageHistorySeries[] = [
  {
    id: "relay-a-main-remaining",
    label: "主套餐余额",
    unit: "CNY",
    metric: "remaining",
    points: [{ at: "2026-08-31T08:00:00Z", value: 1024.5 }],
  },
  {
    id: "relay-a-extra-used",
    label: "附加套餐已用",
    unit: "USD",
    metric: "used",
    points: [{ at: "2026-08-31T08:00:00Z", value: 2 }],
  },
];

function callsFor(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command);
}

function mockProviderCommands(summary: UsageSummary | Error, history = providerHistory) {
  invokeMock.mockImplementation((command) => {
    if (command === "get_usage_history") return Promise.resolve(history) as never;
    if (command === "query_profile_usage") {
      return summary instanceof Error ? Promise.reject(summary) as never : Promise.resolve(summary) as never;
    }
    return Promise.reject(new Error(`unexpected command: ${command}`)) as never;
  });
}

describe("ProviderUsagePanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("runs the configured query on expansion and shows every named reading", async () => {
    const user = userEvent.setup();
    mockProviderCommands(successfulSummary);
    render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );

    expect((await screen.findAllByText("1,024.5 CNY")).length).toBeGreaterThan(0);
    expect(screen.getByRole("table", { name: "中继 A 用量读数" })).toBeInTheDocument();

    const mainRow = screen.getByRole("row", { name: /主套餐/ });
    expect(within(mainRow).getByText("1,024.5 CNY")).toBeInTheDocument();
    expect(within(mainRow).getByText("24 CNY")).toBeInTheDocument();
    expect(within(mainRow).getByText("1,048.5 CNY")).toBeInTheDocument();
    const extraRow = screen.getByRole("row", { name: /附加套餐/ });
    expect(within(extraRow).getByText("8 USD")).toBeInTheDocument();
    expect(within(extraRow).getByText("—")).toBeInTheDocument();
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    expect(invokeMock).toHaveBeenCalledWith("query_profile_usage", { profileId: "relay-a" });
    await waitFor(() => expect(document.querySelector(".recharts-line")).not.toBeNull());
    expect(within(screen.getAllByRole("figure", { name: /余额趋势/ })[0]).getAllByText("主套餐")).toHaveLength(2);
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(2));

    await user.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(callsFor("query_profile_usage")).toHaveLength(2));
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(3));
  });

  it("keeps the configured query error visible without refreshing history", async () => {
    mockProviderCommands(new Error("额度接口返回 403"));
    render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("额度接口返回 403");
    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(1));
  });

  it("clears the displayed trend and reloads history when the saved query changes", async () => {
    let historyReads = 0;
    invokeMock.mockImplementation((command) => {
      if (command === "get_usage_history") {
        historyReads += 1;
        return Promise.resolve(historyReads === 1 ? providerHistory : []) as never;
      }
      if (command === "query_profile_usage") return Promise.reject(new Error("额度接口返回 403")) as never;
      return Promise.reject(new Error(`unexpected command: ${command}`)) as never;
    });
    const { rerender } = render(
      <ProviderUsagePanel id="provider-usage-relay-a" profile={profile} />,
    );

    await waitFor(() => expect(document.querySelector(".recharts-line")).not.toBeNull());

    rerender(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={{
          ...profile,
          usageQuery: { ...usageQuery, url: "{{baseUrl}}/v2/user/balance" },
        }}
      />,
    );

    await waitFor(() => expect(callsFor("get_usage_history")).toHaveLength(2));
    expect(document.querySelector(".recharts-line")).toBeNull();
    expect(screen.getByText("成功读取后会在这里显示趋势。")).toBeInTheDocument();
  });

  it("shows remaining and used trends together when plans provide different metrics", async () => {
    mockProviderCommands(new Error("额度接口返回 403"), mixedMetricHistory);
    render(<ProviderUsagePanel id="provider-usage-relay-a" profile={profile} />);

    await waitFor(() => expect(document.querySelectorAll(".recharts-line")).toHaveLength(2));
    expect(screen.getByRole("heading", { name: "余额趋势" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "已用趋势" })).toBeInTheDocument();
  });

  it("opens the dedicated query workspace through its edit action", async () => {
    const user = userEvent.setup();
    const onConfigure = vi.fn();
    mockProviderCommands({
      readings: [{ remaining: null, used: null, total: null, unit: null }],
      at: "2026-08-31T08:00:00Z",
    });
    render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
        onConfigure={onConfigure}
      />,
    );

    await user.click(screen.getByRole("button", { name: "编辑查询" }));
    expect(onConfigure).toHaveBeenCalledWith(profile);
  });

  it("omits a progress rail when the response cannot produce a ratio", async () => {
    mockProviderCommands({
      readings: [{ planName: "未知额度", remaining: 12, used: null, total: null, unit: "次" }],
      at: "2026-08-31T08:00:00Z",
    });
    render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );

    const unknownRow = await screen.findByRole("row", { name: /未知额度/ });
    expect(within(unknownRow).getByText("12 次")).toBeInTheDocument();
    expect(within(unknownRow).getAllByText("—")).toHaveLength(3);
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("re-queries on the configured auto-refresh interval", async () => {
    vi.useFakeTimers();
    mockProviderCommands({
      readings: [{ planName: "主套餐", remaining: 1, used: 1, total: 2, unit: "CNY" }],
      at: "2026-08-31T08:00:00Z",
    });
    try {
      render(
        <ProviderUsagePanel
          id="provider-usage-relay-a"
          profile={{ ...profile, usageQuery: { ...usageQuery, refreshIntervalMinutes: 2 } }}
        />,
      );
      expect(callsFor("query_profile_usage")).toHaveLength(1);

      act(() => {
        vi.advanceTimersByTime(2 * 60_000);
      });
      expect(callsFor("query_profile_usage")).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("stays manual when the auto-refresh interval is disabled", async () => {
    vi.useFakeTimers();
    mockProviderCommands({ readings: [], at: "2026-08-31T08:00:00Z" });
    try {
      render(
        <ProviderUsagePanel
          id="provider-usage-relay-a"
          profile={profile}
        />,
      );

      act(() => {
        vi.advanceTimersByTime(30 * 60_000);
      });
      expect(callsFor("query_profile_usage")).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("re-queries when the profile changes while the first read is in flight", async () => {
    let resolveFirst: (value: UsageSummary) => void = () => {};
    invokeMock.mockImplementation((command, args) => {
      if (command === "get_usage_history") return Promise.resolve(providerHistory) as never;
      if (command === "query_profile_usage") {
        const profileId = (args as { profileId?: string } | undefined)?.profileId;
        return profileId === "relay-a"
          ? new Promise<UsageSummary>((resolve) => { resolveFirst = resolve; }) as never
          : Promise.resolve({ readings: [], at: "2026-08-31T08:00:00Z" }) as never;
      }
      return Promise.reject(new Error(`unexpected command: ${command}`)) as never;
    });

    const { rerender } = render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );
    expect(callsFor("query_profile_usage")).toHaveLength(1);

    rerender(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={{ ...profile, id: "relay-b", name: "中继 B" }}
      />,
    );

    expect(callsFor("query_profile_usage")).toHaveLength(2);
    expect(invokeMock).toHaveBeenLastCalledWith("query_profile_usage", { profileId: "relay-b" });
    await waitFor(() =>
      expect(screen.queryByText("正在读取已配置的用量…")).not.toBeInTheDocument(),
    );

    resolveFirst({ readings: [], at: "2026-08-31T08:00:00Z" });
    await act(async () => {});
  });
});
