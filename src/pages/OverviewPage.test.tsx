import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OverviewPage } from "./OverviewPage";
import type { SessionScan } from "../api/client";
import { timeLabel } from "../lib/time";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const scan: SessionScan = {
  sessions: [
    {
      app: "codex",
      sessionId: "codex-older",
      title: "不应出现在概览的标题",
      summary: "不应出现在概览的摘要",
      projectDir: "C:/work/agent-switchboard",
      createdAt: "2026-08-28T08:00:00Z",
      lastActiveAt: "2026-08-28T09:00:00Z",
      resumeCommand: "codex resume codex-older",
    },
    {
      app: "claude",
      sessionId: "claude-1",
      title: "Claude 标题",
      summary: "Claude 摘要",
      projectDir: null,
      createdAt: "2026-08-27T08:00:00Z",
      lastActiveAt: null,
      resumeCommand: "claude --resume claude-1",
    },
    {
      app: "codex",
      sessionId: "codex-newer",
      title: "第二个不应出现的标题",
      summary: "第二个不应出现的摘要",
      projectDir: "C:/work/other",
      createdAt: "2026-08-29T08:00:00Z",
      lastActiveAt: "2026-08-29T09:00:00Z",
      resumeCommand: "codex resume codex-newer",
    },
  ],
  issues: [{ app: "claude", message: "一个历史目录不可读" }],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

function renderOverview(onOpenSessions = vi.fn()) {
  render(
    <OverviewPage
      statuses={[]}
      locks={{}}
      selectedProfile={null}
      canSwitch={false}
      busy={false}
      relayHidden
      onPreview={() => {}}
      onRequestSwitch={() => {}}
      onRefresh={() => {}}
      onRecoverLock={() => {}}
      onOpenSessions={onOpenSessions}
    />,
  );
  return onOpenSessions;
}

describe("OverviewPage local session overview", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("waits for an explicit read, aggregates each client, and keeps session content out", async () => {
    invokeMock.mockResolvedValue(scan);
    const onOpenSessions = renderOverview();
    const user = userEvent.setup();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByText("尚未读取本机会话")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "读取本机会话" }));

    const codex = await screen.findByRole("article", { name: "Codex 本机会话" });
    const claude = screen.getByRole("article", { name: "Claude 本机会话" });
    expect(within(codex).getByText("2 个本机会话")).toBeInTheDocument();
    expect(within(codex).getByText(timeLabel("2026-08-29T09:00:00Z"))).toBeInTheDocument();
    expect(within(claude).getByText("1 个本机会话")).toBeInTheDocument();
    expect(within(claude).getByText("没有可读取的记录")).toBeInTheDocument();
    expect(screen.getByText("一个历史目录不可读")).toBeInTheDocument();
    expect(screen.queryByText("不应出现在概览的标题")).not.toBeInTheDocument();
    expect(screen.queryByText("C:/work/agent-switchboard")).not.toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("list_sessions");

    await user.click(screen.getByRole("button", { name: "查看会话" }));
    expect(onOpenSessions).toHaveBeenCalledOnce();
  });

  it("reports a failed read and lets the user retry", async () => {
    invokeMock.mockRejectedValueOnce(new Error("会话目录暂不可读"));
    invokeMock.mockResolvedValueOnce({ sessions: [], issues: [] } satisfies SessionScan);
    const user = userEvent.setup();
    renderOverview();

    await user.click(screen.getByRole("button", { name: "读取本机会话" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取本机会话：会话目录暂不可读");

    await user.click(screen.getByRole("button", { name: "读取本机会话" }));
    expect(await screen.findAllByText("0 个本机会话")).toHaveLength(2);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("shares one in-flight explicit read", async () => {
    const pending = deferred<SessionScan>();
    invokeMock.mockReturnValue(pending.promise);
    renderOverview();

    const button = screen.getByRole("button", { name: "读取本机会话" });
    await act(async () => {
      fireEvent.click(button);
      fireEvent.click(button);
    });
    expect(invokeMock).toHaveBeenCalledOnce();

    await act(async () => {
      pending.resolve({ sessions: [], issues: [] });
    });
    expect(await screen.findAllByText("0 个本机会话")).toHaveLength(2);
  });
});
