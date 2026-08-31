import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SessionManager } from "./SessionManager";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

const scan = {
  sessions: [
    {
      app: "codex",
      sessionId: "codex-1",
      title: "修复供应商预览",
      summary: "检查变更预览与候选文件",
      projectDir: "C:/work/agent-switchboard",
      createdAt: "2026-08-28T08:00:00Z",
      lastActiveAt: "2026-08-28T09:00:00Z",
      resumeCommand: "codex resume codex-1",
    },
    {
      app: "claude",
      sessionId: "claude-2",
      title: "整理会话记录",
      summary: "确认项目目录与恢复命令",
      projectDir: null,
      createdAt: "2026-08-27T08:00:00Z",
      lastActiveAt: "2026-08-27T09:00:00Z",
      resumeCommand: "claude --resume claude-2",
    },
  ],
  issues: [{ app: "claude", message: "一个历史目录不可读" }],
} as const;

function primeBackend() {
  invokeMock.mockImplementation((command: string, args?: unknown) => {
    if (command === "list_sessions") return Promise.resolve(scan);
    if (command === "get_session_messages") {
      if ((args as { app?: string }).app === "claude") {
        return Promise.resolve([{ role: "user", content: "整理记录", at: null }]);
      }
      return Promise.resolve([
        { role: "user", content: "修复预览", at: "2026-08-28T08:00:00Z" },
        { role: "assistant", content: "已完成", at: "2026-08-28T08:01:00Z" },
      ]);
    }
    if (command === "resume_session") {
      return Promise.resolve({ command: "codex resume codex-1", usedProjectDir: true });
    }
    return Promise.resolve(undefined);
  });
}

describe("SessionManager", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("scans read-only sessions and combines provider filtering with search", async () => {
    primeBackend();
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    expect(await screen.findByText("修复供应商预览")).toBeInTheDocument();
    expect(screen.getByText("Claude Code：一个历史目录不可读")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_sessions");

    await user.click(screen.getByRole("radio", { name: "Claude Code" }));
    expect(screen.getByText("整理会话记录")).toBeInTheDocument();
    expect(screen.queryByText("修复供应商预览")).not.toBeInTheDocument();

    await user.clear(screen.getByRole("textbox", { name: "搜索会话" }));
    await user.type(screen.getByRole("textbox", { name: "搜索会话" }), "不存在");
    expect(screen.getByText("未找到匹配的 Codex 或 Claude Code 会话")).toBeInTheDocument();
  });

  it("shows a recoverable scan error without retaining stale session data", async () => {
    invokeMock.mockRejectedValueOnce(new Error("会话目录暂不可读"));
    render(<SessionManager active />);

    expect(await screen.findByRole("alert")).toHaveTextContent("会话目录暂不可读");
    expect(screen.getByText("未找到匹配的 Codex 或 Claude Code 会话")).toBeInTheDocument();
  });

  it("loads a transcript and starts its controlled terminal resume command", async () => {
    primeBackend();
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
    const detail = await screen.findByRole("region", { name: "会话详情" });
    expect(within(detail).getByText("修复预览")).toBeInTheDocument();
    expect(within(detail).getByText("已完成")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_session_messages", {
      app: "codex",
      sessionId: "codex-1",
    });

    await user.click(within(detail).getByRole("button", { name: "在命令提示符中恢复" }));
    expect(invokeMock).toHaveBeenCalledWith("resume_session", {
      app: "codex",
      sessionId: "codex-1",
    });
    expect(within(detail).getByText("已在新命令提示符窗口中恢复会话")).toBeInTheDocument();

    await user.click(within(detail).getByRole("button", { name: "复制恢复命令" }));
    expect(within(detail).getByText("已复制恢复命令")).toBeInTheDocument();
  });

  it("disables directory copying when a session has no recorded project directory", async () => {
    primeBackend();
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /整理会话记录/ }));
    const detail = await screen.findByRole("region", { name: "会话详情" });
    expect(within(detail).getByRole("button", { name: "复制工作目录" })).toBeDisabled();
  });

  it("does not scan while inactive and starts scanning on activation", async () => {
    primeBackend();
    const { rerender } = render(<SessionManager active={false} />);

    expect(invokeMock).not.toHaveBeenCalled();

    rerender(<SessionManager active />);
    expect(await screen.findByText("修复供应商预览")).toBeInTheDocument();
    const scans = invokeMock.mock.calls.filter(([command]) => command === "list_sessions");
    expect(scans).toHaveLength(1);
  });

  it("shares one in-flight scan across rapid re-activation instead of piling up", async () => {
    primeBackend();
    const pending = deferred<typeof scan>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_sessions") return pending.promise;
      return Promise.resolve(undefined);
    });

    const { rerender } = render(<SessionManager active />);
    rerender(<SessionManager active={false} />);
    rerender(<SessionManager active />);
    rerender(<SessionManager active={false} />);
    rerender(<SessionManager active />);

    const scans = invokeMock.mock.calls.filter(([command]) => command === "list_sessions");
    expect(scans).toHaveLength(1);

    await act(async () => {
      pending.resolve(scan);
    });
    expect(await screen.findByText("修复供应商预览")).toBeInTheDocument();
  });
});
