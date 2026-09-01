import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderUsagePanel } from "./ProviderUsagePanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const profile = {
  id: "relay-a",
  app: "codex" as const,
  routeMode: "custom" as const,
  name: "中继 A",
  model: null,
  baseUrl: "https://relay.example/v1",
  apiKey: "test-key",
  modelOptions: null,
};

const query = {
  kind: "script" as const,
  source: "({ request() {}, extract() {} })",
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
    render(<ProviderUsagePanel id="provider-usage-relay-a" profile={profile} query={query} />);

    expect(await screen.findByText("1,024.5")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "用量" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "主套餐" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "附加套餐" })).toBeInTheDocument();
    expect(screen.getByText("已用 24")).toBeInTheDocument();
    expect(screen.getByText("总量 1,048.5 CNY")).toBeInTheDocument();
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    expect(invokeMock).toHaveBeenCalledWith("query_profile_usage", { profileId: "relay-a" });

    await user.click(screen.getByRole("button", { name: "刷新" }));
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("keeps the configured query error visible in the expanded card", async () => {
    invokeMock.mockRejectedValueOnce(new Error("额度接口返回 403"));
    render(<ProviderUsagePanel id="provider-usage-relay-a" profile={profile} query={query} />);

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
        query={query}
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
    render(<ProviderUsagePanel id="provider-usage-relay-a" profile={profile} query={query} />);

    expect(await screen.findByText("未知额度")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });
});
