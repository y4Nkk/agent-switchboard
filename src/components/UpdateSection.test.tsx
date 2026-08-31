import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { UpdateCheck } from "../api/client";
import { UpdateSection } from "./UpdateSection";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

const available: UpdateCheck = {
  currentVersion: "0.1.0",
  latestVersion: "v0.2.0",
  updateAvailable: true,
  releaseUrl: "https://github.com/y4Nkk/agent-switchboard/releases/tag/v0.2.0",
  checkedAt: "2026-08-31T08:00:00Z",
};

const upToDate: UpdateCheck = { ...available, latestVersion: "0.1.0", updateAvailable: false };

describe("UpdateSection", () => {
  it("shows the manual check affordance before any check ran", () => {
    render(<UpdateSection result={null} busy={false} onCheck={() => {}} />);

    expect(screen.getByText("检查新版本")).toBeInTheDocument();
    expect(screen.getByText("从 GitHub 发布页检查 Agent Switchboard 的最新版本")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "打开下载页面" })).not.toBeInTheDocument();
  });

  it("emits the check request on demand", async () => {
    const user = userEvent.setup();
    const onCheck = vi.fn();
    render(<UpdateSection result={null} busy={false} onCheck={onCheck} />);

    await user.click(screen.getByRole("button", { name: "检查更新" }));
    expect(onCheck).toHaveBeenCalledTimes(1);
  });

  it("disables the check button while the frame is busy", () => {
    render(<UpdateSection result={available} busy onCheck={() => {}} />);

    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();
  });

  it("reports an up-to-date result without a download entry", () => {
    render(<UpdateSection result={upToDate} busy={false} onCheck={() => {}} />);

    expect(screen.getByText("已是最新版本")).toBeInTheDocument();
    expect(screen.getByText(/当前版本 0\.1\.0 · 检查于/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "打开下载页面" })).not.toBeInTheDocument();
  });

  it("offers the release page when a newer version is found", async () => {
    vi.mocked(openUrl).mockClear();
    const user = userEvent.setup();
    render(<UpdateSection result={available} busy={false} onCheck={() => {}} />);

    expect(screen.getByText("发现新版本 v0.2.0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "打开下载页面" }));
    expect(openUrl).toHaveBeenCalledWith(available.releaseUrl);
  });
});
