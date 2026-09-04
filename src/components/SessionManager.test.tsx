import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SessionManager } from "./SessionManager";
import { Toaster } from "./Toaster";

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

    await user.click(within(detail).getByRole("button", { name: "在终端中恢复" }));
    expect(invokeMock).toHaveBeenCalledWith("resume_session", {
      app: "codex",
      sessionId: "codex-1",
    });
    expect(within(detail).getByText("已在新终端窗口中恢复会话")).toBeInTheDocument();

    await user.click(within(detail).getByRole("button", { name: "复制恢复命令" }));
    expect(within(detail).getByText("已复制恢复命令")).toBeInTheDocument();
  });

  it("does not attach an earlier resume result to a newly selected session", async () => {
    const pendingResume = deferred<{ command: string; usedProjectDir: boolean }>();
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "list_sessions") return Promise.resolve(scan);
      if (command === "get_session_messages") {
        return Promise.resolve([
          {
            role: "user",
            content: `内容 ${(args as { sessionId?: string }).sessionId}`,
            at: null,
          },
        ]);
      }
      if (command === "resume_session") return pendingResume.promise;
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
    const firstDetail = await screen.findByRole("region", { name: "会话详情" });
    await user.click(within(firstDetail).getByRole("button", { name: "在终端中恢复" }));

    await user.click(screen.getByRole("button", { name: /整理会话记录/ }));
    const currentDetail = screen.getByRole("region", { name: "会话详情" });
    expect(within(currentDetail).getByRole("button", { name: "在终端中恢复" })).not.toBeDisabled();

    await act(async () => {
      pendingResume.resolve({ command: "codex resume codex-1", usedProjectDir: true });
    });
    expect(within(currentDetail).queryByText("已在新终端窗口中恢复会话")).not.toBeInTheDocument();
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

  it("collapses long messages and expands them back on demand", async () => {
    const longContent = "甲".repeat(3200);
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_sessions") return Promise.resolve(scan);
      if (command === "get_session_messages") {
        return Promise.resolve([{ role: "user", content: longContent, at: null }]);
      }
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
    const detail = await screen.findByRole("region", { name: "会话详情" });

    const expand = within(detail).getByRole("button", { name: /展开完整内容/ });
    expect(expand).toHaveAttribute("aria-expanded", "false");
    expect(within(detail).getByText(`${"甲".repeat(1500)}…`)).toBeInTheDocument();
    expect(within(detail).queryByText(longContent)).not.toBeInTheDocument();

    await user.click(expand);
    expect(within(detail).getByText(longContent)).toBeInTheDocument();
    const collapse = within(detail).getByRole("button", { name: "收起" });
    expect(collapse).toHaveAttribute("aria-expanded", "true");

    await user.click(collapse);
    expect(within(detail).queryByText(longContent)).not.toBeInTheDocument();
    expect(within(detail).getByRole("button", { name: /展开完整内容/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("keeps Codex-injected payloads out of the outline and surfaces the embedded IDE prompt", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_sessions") return Promise.resolve(scan);
      if (command === "get_session_messages") {
        return Promise.resolve([
          {
            role: "user",
            content: "# AGENTS.md instructions for F:/work\n全文指令转储",
            at: null,
          },
          {
            role: "user",
            content: "<environment_context>\n工作目录与环境变量",
            at: null,
          },
          {
            role: "user",
            content:
              "# Context from my IDE setup:\n打开的文件内容里可能出现标题\n## My request for Codex:\n修复登录跳转\n",
            at: null,
          },
          { role: "user", content: "直接提问的正文", at: null },
          { role: "user", content: "后续追问", at: null },
        ]);
      }
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
    const outline = await screen.findByRole("navigation", { name: "消息目录" });
    const entries = within(outline).getAllByRole("button");

    expect(entries).toHaveLength(3);
    expect(within(entries[0]).getByText(/修复登录跳转/)).toBeInTheDocument();
    expect(within(entries[1]).getByText("直接提问的正文")).toBeInTheDocument();
    expect(within(entries[2]).getByText("后续追问")).toBeInTheDocument();
    expect(within(outline).queryByText(/AGENTS\.md instructions/)).not.toBeInTheDocument();
    expect(within(outline).queryByText(/environment_context/)).not.toBeInTheDocument();
  });

  it("still outlines Claude user messages that merely look like Codex payloads", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_sessions") return Promise.resolve(scan);
      if (command === "get_session_messages") {
        return Promise.resolve([
          { role: "user", content: "# AGENTS.md instructions for demo", at: null },
          { role: "assistant", content: "收到", at: null },
          { role: "user", content: "第二个问题", at: null },
          { role: "user", content: "第三个问题", at: null },
        ]);
      }
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /整理会话记录/ }));
    const outline = await screen.findByRole("navigation", { name: "消息目录" });
    const entries = within(outline).getAllByRole("button");

    expect(entries).toHaveLength(3);
    expect(within(entries[0]).getByText(/AGENTS\.md instructions for demo/)).toBeInTheDocument();
    expect(within(entries[1]).getByText("第二个问题")).toBeInTheDocument();
    expect(within(entries[2]).getByText("第三个问题")).toBeInTheDocument();
  });

  it("jumps from the message outline to the target message and highlights it", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_sessions") return Promise.resolve(scan);
      if (command === "get_session_messages") {
        return Promise.resolve([
          { role: "user", content: "第一条提问", at: null },
          { role: "assistant", content: "回答一", at: null },
          { role: "user", content: "第二条提问", at: null },
          { role: "assistant", content: "回答二", at: null },
          { role: "user", content: "第三条提问", at: null },
        ]);
      }
      return Promise.resolve(undefined);
    });
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const user = userEvent.setup({ writeToClipboard: false });
    try {
      render(<SessionManager active />);

      await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
      const outline = await screen.findByRole("navigation", { name: "消息目录" });
      const entries = within(outline).getAllByRole("button");
      expect(entries).toHaveLength(3);
      expect(within(entries[0]).getByText("第一条提问")).toBeInTheDocument();

      await user.click(entries[2]);
      expect(scrollIntoView).toHaveBeenCalled();
      expect(
        document.querySelector('.asb-session-message[data-index="4"]'),
      ).toHaveClass("is-target");
      expect(
        document.querySelector('.asb-session-message[data-index="0"]'),
      ).not.toHaveClass("is-target");
    } finally {
      delete (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView;
    }
  });

  it("copies one message and reports the result as a toast", async () => {
    primeBackend();
    const user = userEvent.setup({ writeToClipboard: false });
    render(
      <>
        <Toaster />
        <SessionManager active />
      </>,
    );

    await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
    await screen.findByText("修复预览");

    await user.click(screen.getAllByRole("button", { name: "复制" })[0]);
    expect(await screen.findByText("已复制消息内容")).toBeInTheDocument();
  });

  it("serves the cached list on re-activation inside the TTL without rescanning", async () => {
    primeBackend();
    const { rerender } = render(<SessionManager active />);
    expect(await screen.findByText("修复供应商预览")).toBeInTheDocument();

    rerender(<SessionManager active={false} />);
    rerender(<SessionManager active />);

    const scans = invokeMock.mock.calls.filter(([command]) => command === "list_sessions");
    expect(scans).toHaveLength(1);
    expect(screen.getByText("修复供应商预览")).toBeInTheDocument();
    expect(screen.queryByText("正在扫描本地会话")).not.toBeInTheDocument();
  });

  it("rescans in the background once the cache goes stale while keeping the list visible", async () => {
    primeBackend();
    const now = vi.spyOn(Date, "now");
    const { rerender } = render(<SessionManager active />);
    expect(await screen.findByText("修复供应商预览")).toBeInTheDocument();

    now.mockReturnValue(Date.now() + 31_000);
    rerender(<SessionManager active={false} />);
    rerender(<SessionManager active />);

    const scans = invokeMock.mock.calls.filter(([command]) => command === "list_sessions");
    expect(scans).toHaveLength(2);
    expect(screen.getByText("修复供应商预览")).toBeInTheDocument();
    expect(screen.queryByText("正在扫描本地会话")).not.toBeInTheDocument();
    now.mockRestore();
  });

  it("keeps the displayed list when a stale background rescan fails", async () => {
    primeBackend();
    const now = vi.spyOn(Date, "now");
    const { rerender } = render(<SessionManager active />);
    expect(await screen.findByText("修复供应商预览")).toBeInTheDocument();

    invokeMock.mockImplementationOnce(() => Promise.reject(new Error("目录暂时不可读")));
    now.mockReturnValue(Date.now() + 31_000);
    rerender(<SessionManager active={false} />);
    rerender(<SessionManager active />);

    expect(await screen.findByRole("alert")).toHaveTextContent("目录暂时不可读");
    expect(screen.getByText("修复供应商预览")).toBeInTheDocument();
    now.mockRestore();
  });

  it("reuses cached transcripts within the TTL instead of refetching", async () => {
    primeBackend();
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    await user.click(await screen.findByRole("button", { name: /修复供应商预览/ }));
    expect(await screen.findByText("修复预览")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /整理会话记录/ }));
    expect(await screen.findByText("整理记录")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /修复供应商预览/ }));

    const detail = screen.getByRole("region", { name: "会话详情" });
    expect(within(detail).getByText("修复预览")).toBeInTheDocument();
    expect(within(detail).queryByText("正在读取会话内容")).not.toBeInTheDocument();
    const fetches = invokeMock.mock.calls.filter(([command]) => command === "get_session_messages");
    expect(fetches).toHaveLength(2);
  });

  it("evicts the oldest cached transcript beyond the retention cap and refetches it", async () => {
    const capped = {
      sessions: Array.from({ length: 17 }, (_, index) => ({
        app: "codex" as const,
        sessionId: `cap-${index}`,
        title: `会话 ${String(index).padStart(2, "0")}`,
        summary: "上限契约",
        projectDir: null,
        createdAt: "2026-08-28T08:00:00Z",
        lastActiveAt: "2026-08-28T09:00:00Z",
        resumeCommand: `codex resume cap-${index}`,
      })),
      issues: [],
    };
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "list_sessions") return Promise.resolve(capped);
      if (command === "get_session_messages") {
        return Promise.resolve([
          {
            role: "user",
            content: `内容 ${(args as { sessionId?: string }).sessionId}`,
            at: null,
          },
        ]);
      }
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup({ writeToClipboard: false });
    render(<SessionManager active />);

    for (let index = 0; index < 17; index += 1) {
      await user.click(
        await screen.findByRole("button", { name: new RegExp(`^会话 ${String(index).padStart(2, "0")}`) }),
      );
    }
    expect(await screen.findByText("内容 cap-16")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^会话 00/ }));
    expect(await screen.findByText("内容 cap-0")).toBeInTheDocument();
    const fetches = invokeMock.mock.calls.filter(([command]) => command === "get_session_messages");
    expect(fetches).toHaveLength(18);
  });
});
