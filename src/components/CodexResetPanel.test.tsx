import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { CodexResetRead, CodexResetStatus } from "../api/client";
import { timeLabel } from "../lib/time";
import { CodexResetPanel } from "./CodexResetPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const postUrl = "https://x.com/thsottiaux/status/2094252447271366730";
const feedUrl = "https://www.codexrunway.com/api/status.json";
const ready: CodexResetStatus = {
  sourceUrl: feedUrl,
  feedStatus: "ok",
  generatedAt: "2026-08-31T03:08:02.232Z",
  lastSuccessfulCheckAt: "2026-08-31T03:08:02.232Z",
  checkedAt: "2026-08-31T03:10:00.000Z",
  latestConfirmedReset: {
    announcedAt: "2026-08-31T02:34:27Z",
    effectiveAt: null,
    schedulePrecision: null,
    confidence: 0.98,
  },
  nextScheduledReset: {
    announcedAt: "2026-08-31T03:00:00Z",
    effectiveAt: "2026-08-31T09:00:00Z",
    schedulePrecision: "datetime",
    confidence: 0.84,
  },
  latestRelevantTiboPost: {
    announcedAt: "2026-08-31T02:34:27Z",
    text: "A public reset-related message from Tibo.",
    url: postUrl,
  },
  sourceWarning: null,
};

function live(status: CodexResetStatus, cacheWarning: string | null = null): CodexResetRead {
  return { status, freshness: "live", cacheWarning };
}

function cached(status: CodexResetStatus): CodexResetRead {
  return { status, freshness: "cached", cacheWarning: null };
}

describe("CodexResetPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.mocked(openUrl).mockReset();
  });

  it("reads a local cache on mount without requesting public reset signals", async () => {
    invokeMock.mockResolvedValueOnce(null);
    render(<CodexResetPanel />);

    expect(await screen.findByText("尚无本地缓存。手动刷新以读取公开重置信号。")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_cached_codex_reset_status");
    expect(invokeMock).not.toHaveBeenCalledWith("check_codex_reset_status");
  });

  it("renders the completed reset, scheduled reset, and latest related Tibo post", async () => {
    invokeMock.mockResolvedValueOnce(null).mockResolvedValueOnce(live(ready, "本地缓存未更新"));
    const user = userEvent.setup();
    render(<CodexResetPanel />);

    await screen.findByText("尚无本地缓存。手动刷新以读取公开重置信号。");
    await user.click(screen.getByRole("button", { name: "刷新重置信号" }));

    expect(await screen.findByText("已确认全局重置")).toBeInTheDocument();
    expect(
      within(screen.getByRole("article", { name: "是否重置" })).getByText(
        timeLabel(ready.latestConfirmedReset!.announcedAt),
      ),
    ).toBeInTheDocument();
    expect(
      within(screen.getByRole("article", { name: "预计重置" })).getByText(
        timeLabel(ready.nextScheduledReset!.effectiveAt!),
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("精确时间预告 · 信心 84%")).toBeInTheDocument();
    expect(screen.getByText(ready.latestRelevantTiboPost!.text)).toBeInTheDocument();
    expect(screen.getByText("刚刚刷新")).toBeInTheDocument();
    expect(screen.getByText("本地缓存未更新")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_cached_codex_reset_status");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "check_codex_reset_status");

    await user.click(screen.getByRole("button", { name: "查看原帖" }));
    expect(openUrl).toHaveBeenCalledWith(postUrl);

    const feedLink = screen.getByRole("link", { name: feedUrl });
    expect(feedLink).toHaveAttribute("href", feedUrl);
    await user.click(feedLink);
    expect(openUrl).toHaveBeenCalledWith(feedUrl);
  });

  it("distinguishes no schedule from a degraded public source", async () => {
    invokeMock
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(live({
        ...ready,
        feedStatus: "degraded",
        nextScheduledReset: null,
        latestRelevantTiboPost: null,
        sourceWarning: "公开信号源未确认正常，展示的内容可能不是最新状态。",
      } satisfies CodexResetStatus));
    const user = userEvent.setup();
    render(<CodexResetPanel />);

    await screen.findByText("尚无本地缓存。手动刷新以读取公开重置信号。");
    await user.click(screen.getByRole("button", { name: "刷新重置信号" }));

    expect(await screen.findByText("暂无公告预计")).toBeInTheDocument();
    expect(screen.getByText("暂无相关动态")).toBeInTheDocument();
    expect(screen.getByText("公开信号源未确认正常，展示的内容可能不是最新状态。")).toBeInTheDocument();
  });

  it("keeps a cached signal visible when refresh fails and permits a later retry", async () => {
    invokeMock
      .mockResolvedValueOnce(cached(ready))
      .mockRejectedValueOnce(new Error("公开 feed 暂不可用"))
      .mockResolvedValueOnce(live(ready));
    const user = userEvent.setup();
    render(<CodexResetPanel />);

    expect(await screen.findByText("本地缓存")).toBeInTheDocument();
    expect(screen.getByText("缓存于", { exact: false })).toBeInTheDocument();
    expect(screen.getByText("已确认全局重置")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新重置信号" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "无法刷新公开重置信号：公开 feed 暂不可用；仍在显示上次成功读取的数据。",
    );
    expect(screen.getByText("已确认全局重置")).toBeInTheDocument();
    expect(screen.getByText("本地缓存")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新重置信号" }));
    expect(await screen.findByText("已确认全局重置")).toBeInTheDocument();
    expect(screen.getByText("刚刚刷新")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledTimes(3);
  });
});
