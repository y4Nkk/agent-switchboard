import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderUsagePanel } from "./ProviderUsagePanel";
import type { UsageSummary } from "../api/client";

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

describe("ProviderUsagePanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("runs the configured query on expansion and shows every named reading", async () => {
    const user = userEvent.setup();
    invokeMock.mockResolvedValue({
      readings: [
        { planName: "主套餐", remaining: 1024.5, used: 24, total: 1048.5, unit: "CNY" },
        { planName: "附加套餐", remaining: 8, used: null, total: 10, unit: "USD" },
      ],
      at: "2026-08-31T08:00:00Z",
    });
    render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );

    expect(await screen.findByText("1,024.5 CNY")).toBeInTheDocument();
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

    await user.click(screen.getByRole("button", { name: "刷新" }));
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("keeps the configured query error visible in the expanded card", async () => {
    invokeMock.mockRejectedValueOnce(new Error("额度接口返回 403"));
    render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("额度接口返回 403");
  });

  it("opens the dedicated query workspace through its edit action", async () => {
    const user = userEvent.setup();
    const onConfigure = vi.fn();
    invokeMock.mockResolvedValue({
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
    invokeMock.mockResolvedValue({
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
    invokeMock.mockResolvedValue({
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
      await act(async () => {});
      expect(invokeMock).toHaveBeenCalledTimes(1);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2 * 60_000);
      });
      expect(invokeMock).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("stays manual when the auto-refresh interval is disabled", async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue({ readings: [], at: "2026-08-31T08:00:00Z" });
    try {
      render(
        <ProviderUsagePanel
          id="provider-usage-relay-a"
          profile={profile}
        />,
      );
      await act(async () => {});

      await act(async () => {
        await vi.advanceTimersByTimeAsync(30 * 60_000);
      });
      expect(invokeMock).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("re-queries when the profile changes while the first read is in flight", async () => {
    let resolveFirst: (value: UsageSummary) => void = () => {};
    invokeMock.mockImplementationOnce(
      () => new Promise<UsageSummary>((resolve) => { resolveFirst = resolve; }),
    );
    invokeMock.mockResolvedValue({ readings: [], at: "2026-08-31T08:00:00Z" });

    const { rerender } = render(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={profile}
      />,
    );
    expect(invokeMock).toHaveBeenCalledTimes(1);

    rerender(
      <ProviderUsagePanel
        id="provider-usage-relay-a"
        profile={{ ...profile, id: "relay-b", name: "中继 B" }}
      />,
    );

    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(invokeMock).toHaveBeenLastCalledWith("query_profile_usage", { profileId: "relay-b" });
    await waitFor(() =>
      expect(screen.queryByText("正在读取已配置的用量…")).not.toBeInTheDocument(),
    );

    resolveFirst({ readings: [], at: "2026-08-31T08:00:00Z" });
    await act(async () => {});
  });
});
