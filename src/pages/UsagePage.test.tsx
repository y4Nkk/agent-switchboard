import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ModelUsageRange, ModelUsageRead, ModelUsageReport } from "../api/client";
import { UsagePage } from "./UsagePage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function report(range: ModelUsageRange, model = "gpt-5.3"): ModelUsageReport {
  return {
    range,
    generatedAt: "2026-09-03T01:00:00Z",
    groups: [
      {
        app: "codex",
        model,
        inputTokens: 1200,
        cacheReadInputTokens: 240,
        cacheCreationInputTokens: 60,
        outputTokens: 600,
        totalTokens: 2100,
        sessionCount: 2,
      },
    ],
    days: [
      {
        date: "2026-09-03",
        inputTokens: 1200,
        cacheReadInputTokens: 240,
        cacheCreationInputTokens: 60,
        outputTokens: 600,
        totalTokens: 2100,
      },
    ],
    unassignedTokens: {
      inputTokens: 0,
      cacheReadInputTokens: 0,
      cacheCreationInputTokens: 0,
      outputTokens: 0,
      totalTokens: 0,
    },
    issues: [{ app: "claude", message: "部分本地会话无法读取" }],
  };
}

function read(
  range: ModelUsageRange,
  model = "gpt-5.3",
  freshness: ModelUsageRead["freshness"] = "fresh",
  refreshAfter = new Date(Date.now() + 60_000).toISOString(),
): ModelUsageRead {
  return {
    report: report(range, model),
    freshness,
    refreshAfter,
    cacheWarning: null,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

describe("UsagePage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("在激活时读取本地消耗并渲染模型表格", async () => {
    invokeMock.mockResolvedValue(read("today"));

    render(<UsagePage active />);

    const table = await screen.findByRole("table", { name: "模型消耗" });
    expect(invokeMock).toHaveBeenCalledWith("get_model_usage_report", {
      request: { range: "today", forceRefresh: false },
    });
    expect(within(table).getByText("Codex")).toBeInTheDocument();
    expect(within(table).getByText("gpt-5.3")).toBeInTheDocument();
    expect(within(table).getByText("1,200")).toBeInTheDocument();
    expect(within(table).getByText("2,100")).toBeInTheDocument();
    expect(screen.getByText("Claude：部分本地会话无法读取")).toBeInTheDocument();
    expect(screen.getByRole("figure", { name: "模型消耗日趋势" })).toBeInTheDocument();
    expect(screen.getByRole("figure", { name: "模型消耗构成" })).toBeInTheDocument();
    expect(screen.getByText("每日 Token 趋势")).toBeInTheDocument();
    const summary = screen.getByRole("group", { name: "模型消耗汇总" });
    expect(summary.parentElement).toHaveClass("asb-model-usage-summary");
    expect(summary.parentElement?.parentElement).toHaveClass("asb-model-usage-content");
    expect(within(summary).getByText("新输入")).toBeInTheDocument();
    expect(within(summary).getByText("1.2K")).toBeInTheDocument();
    expect(within(summary).getByText("2.1K")).toBeInTheDocument();
    expect(within(summary).getAllByText("tokens")).toHaveLength(4);
    expect(within(table).getByText("输入（tokens）")).toBeInTheDocument();
    expect(within(table).getByText("1,200").closest("td")).toHaveClass("asb-table-cell");
    expect(within(table).getByText("1,200").closest("td")).not.toHaveClass("asb-model-usage-number");
    expect(document.querySelectorAll(".recharts-pie-sector")).toHaveLength(1);
  });

  it("在更改时间范围时按新范围重新汇总", async () => {
    invokeMock
      .mockResolvedValueOnce(read("today"))
      .mockResolvedValueOnce(read("last7Days", "claude-sonnet-4"));
    const user = userEvent.setup();
    render(<UsagePage active />);

    await screen.findByText("gpt-5.3");
    await user.click(screen.getByLabelText("近 7 天"));

    expect(await screen.findByText("claude-sonnet-4")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenLastCalledWith("get_model_usage_report", {
      request: { range: "last7Days", forceRefresh: false },
    });
  });

  it("不会用迟到的旧范围响应覆盖当前范围", async () => {
    const today = deferred<ModelUsageRead>();
    const last7Days = deferred<ModelUsageRead>();
    invokeMock.mockReturnValueOnce(today.promise).mockReturnValueOnce(last7Days.promise);
    const user = userEvent.setup();
    render(<UsagePage active />);

    await user.click(screen.getByLabelText("近 7 天"));
    await act(async () => {
      last7Days.resolve(read("last7Days", "new-model"));
    });
    expect(await screen.findByText("new-model")).toBeInTheDocument();

    await act(async () => {
      today.resolve(read("today", "old-model"));
    });
    await waitFor(() => expect(screen.queryByText("old-model")).not.toBeInTheDocument());
    expect(screen.getByText("new-model")).toBeInTheDocument();
  });

  it("在未激活时不读取，重新激活后才读取", async () => {
    invokeMock.mockResolvedValue(read("today"));
    const { rerender } = render(<UsagePage active={false} />);

    expect(invokeMock).not.toHaveBeenCalled();
    rerender(<UsagePage active />);

    await screen.findByText("gpt-5.3");
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("重新进入时显示同一范围的本地快照，不重复扫描", async () => {
    invokeMock
      .mockResolvedValueOnce(read("today"))
      .mockResolvedValueOnce(read("today", "gpt-5.3", "cached"));
    const { rerender } = render(<UsagePage active />);

    await screen.findByText("gpt-5.3");
    rerender(<UsagePage active={false} />);
    rerender(<UsagePage active />);

    expect(await screen.findByRole("status")).toHaveTextContent("本地快照");
    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(invokeMock).toHaveBeenLastCalledWith("get_model_usage_report", {
      request: { range: "today", forceRefresh: false },
    });
  });

  it("可见时按后端快照的刷新时间自动重新汇总", async () => {
    vi.useFakeTimers();
    try {
      invokeMock.mockResolvedValue(read("today"));
      render(<UsagePage active />);

      await act(async () => {
        await Promise.resolve();
      });
      expect(invokeMock).toHaveBeenCalledTimes(1);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(60_000);
      });
      expect(invokeMock).toHaveBeenCalledTimes(2);
      expect(invokeMock).toHaveBeenLastCalledWith("get_model_usage_report", {
        request: { range: "today", forceRefresh: true },
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it("手动刷新绕过本地快照，并在失败时保留已显示的结果", async () => {
    invokeMock
      .mockResolvedValueOnce(read("today", "cached-model"))
      .mockRejectedValueOnce(new Error("读取失败"));
    const user = userEvent.setup();
    render(<UsagePage active />);

    expect(await screen.findByText("cached-model")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "刷新" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("读取失败");
    expect(screen.getByText("cached-model")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("手动刷新成功后按新的后端刷新时间重新计时", async () => {
    vi.useFakeTimers();
    try {
      const initialRefreshAfter = new Date(Date.now() + 60_000).toISOString();
      const refreshedRefreshAfter = new Date(Date.now() + 120_000).toISOString();
      invokeMock
        .mockResolvedValueOnce(read("today", "first", "fresh", initialRefreshAfter))
        .mockResolvedValueOnce(read("today", "second", "fresh", refreshedRefreshAfter))
        .mockResolvedValueOnce(read("today", "third"));
      render(<UsagePage active />);

      await act(async () => {
        await Promise.resolve();
      });
      fireEvent.click(screen.getByRole("button", { name: "刷新" }));
      await act(async () => {
        await Promise.resolve();
      });
      expect(screen.getByText("second")).toBeInTheDocument();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(90_000);
      });
      expect(invokeMock).toHaveBeenCalledTimes(2);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(30_000);
      });
      expect(invokeMock).toHaveBeenCalledTimes(3);
      expect(invokeMock).toHaveBeenLastCalledWith("get_model_usage_report", {
        request: { range: "today", forceRefresh: true },
      });
    } finally {
      vi.useRealTimers();
    }
  });
});
