import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ModelUsageRange, ModelUsageReport } from "../api/client";
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
    invokeMock.mockResolvedValue(report("today"));

    render(<UsagePage active />);

    const table = await screen.findByRole("table", { name: "模型消耗" });
    expect(invokeMock).toHaveBeenCalledWith("get_model_usage_report", { range: "today" });
    expect(within(table).getByText("Codex")).toBeInTheDocument();
    expect(within(table).getByText("gpt-5.3")).toBeInTheDocument();
    expect(within(table).getByText("1,200")).toBeInTheDocument();
    expect(within(table).getByText("2,100")).toBeInTheDocument();
    expect(screen.getByText("Claude：部分本地会话无法读取")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "模型消耗日趋势" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "模型消耗构成" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: /新输入，1,200 token/ })).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: /Codex · gpt-5.3 占模型消耗总计的比例/ })).toBeInTheDocument();
  });

  it("在更改时间范围时按新范围重新汇总", async () => {
    invokeMock
      .mockResolvedValueOnce(report("today"))
      .mockResolvedValueOnce(report("last7Days", "claude-sonnet-4"));
    const user = userEvent.setup();
    render(<UsagePage active />);

    await screen.findByText("gpt-5.3");
    await user.click(screen.getByLabelText("近 7 天"));

    expect(await screen.findByText("claude-sonnet-4")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenLastCalledWith("get_model_usage_report", { range: "last7Days" });
  });

  it("不会用迟到的旧范围响应覆盖当前范围", async () => {
    const today = deferred<ModelUsageReport>();
    const last7Days = deferred<ModelUsageReport>();
    invokeMock.mockReturnValueOnce(today.promise).mockReturnValueOnce(last7Days.promise);
    const user = userEvent.setup();
    render(<UsagePage active />);

    await user.click(screen.getByLabelText("近 7 天"));
    await act(async () => {
      last7Days.resolve(report("last7Days", "new-model"));
    });
    expect(await screen.findByText("new-model")).toBeInTheDocument();

    await act(async () => {
      today.resolve(report("today", "old-model"));
    });
    await waitFor(() => expect(screen.queryByText("old-model")).not.toBeInTheDocument());
    expect(screen.getByText("new-model")).toBeInTheDocument();
  });

  it("在未激活时不读取，重新激活后才读取", async () => {
    invokeMock.mockResolvedValue(report("today"));
    const { rerender } = render(<UsagePage active={false} />);

    expect(invokeMock).not.toHaveBeenCalled();
    rerender(<UsagePage active />);

    await screen.findByText("gpt-5.3");
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
