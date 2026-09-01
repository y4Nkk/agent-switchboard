import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodexOfficialQuotaPanel } from "./CodexOfficialQuotaPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

describe("CodexOfficialQuotaPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders the native server windows without receiving OAuth data", async () => {
    const user = userEvent.setup();
    invokeMock.mockResolvedValue({
      status: "available",
      windows: [
        { label: "5 小时", usedPercent: 12.5, resetsAt: "2026-09-01T08:00:00Z" },
        { label: "7 天", usedPercent: 76.25, resetsAt: "2026-09-06T08:00:00Z" },
      ],
      at: "2026-09-01T03:00:00Z",
      stale: false,
    });

    render(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-codex-official"
        profileId="codex-official"
        profileName="Codex 官方登录"
      />,
    );

    expect(await screen.findByRole("heading", { name: "5 小时" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "7 天" })).toBeInTheDocument();
    expect(screen.getByText("13%")).toBeInTheDocument();
    expect(screen.getByText("76%")).toBeInTheDocument();
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    expect(invokeMock).toHaveBeenCalledWith("query_codex_official_quota", {
      profileId: "codex-official",
    });
    expect(invokeMock.mock.calls[0]?.[1]).toEqual({ profileId: "codex-official" });

    await user.click(screen.getByRole("button", { name: "刷新" }));
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("keeps a stale successful read visible beside the recoverable refresh state", async () => {
    invokeMock.mockResolvedValue({
      status: "unavailable",
      windows: [{ label: "5 小时", usedPercent: 38, resetsAt: null }],
      at: "2026-09-01T03:00:00Z",
      stale: true,
    });

    render(
      <CodexOfficialQuotaPanel
        id="codex-official-quota-codex-official"
        profileId="codex-official"
        profileName="Codex 官方登录"
      />,
    );

    expect(await screen.findByText("38%")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("正在显示上次成功读取的额度");
  });

  it("directs a missing native login to the Codex login recovery action", async () => {
    invokeMock.mockResolvedValue({
      status: "signInRequired",
      windows: [],
      at: null,
      stale: false,
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
  });
});
