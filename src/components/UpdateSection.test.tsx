import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Update } from "@tauri-apps/plugin-updater";
import type { UpdateCheck } from "../api/client";
import { UpdateSection } from "./UpdateSection";

const available: UpdateCheck = {
  currentVersion: "0.1.0",
  latestVersion: "0.2.0",
  checkedAt: "2026-08-31T08:00:00Z",
  update: {} as Update,
};

function renderSection(overrides: Partial<Parameters<typeof UpdateSection>[0]> = {}) {
  const props = {
    channel: null,
    result: null,
    busy: false,
    installing: false,
    progress: null,
    checkedAt: null,
    restartRequired: false,
    onCheck: vi.fn(),
    onInstall: vi.fn(),
    onRestart: vi.fn(),
    ...overrides,
  };
  render(<UpdateSection {...props} />);
  return props;
}

describe("UpdateSection", () => {
  it("does not offer the GitHub updater in a Store installation", () => {
    renderSection({ channel: "microsoftStore" });

    expect(screen.getByText("由 Microsoft Store 管理更新")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "检查更新" })).not.toBeInTheDocument();
  });

  it("shows the manual check affordance before any check ran", () => {
    renderSection();

    expect(screen.getByText("检查新版本")).toBeInTheDocument();
    expect(screen.queryByText(/检查于/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "下载并安装" })).not.toBeInTheDocument();
  });

  it("emits the check request on demand", async () => {
    const user = userEvent.setup();
    const props = renderSection();

    await user.click(screen.getByRole("button", { name: "检查更新" }));
    expect(props.onCheck).toHaveBeenCalledTimes(1);
  });

  it("shows the startup lookup state before a result is available", () => {
    renderSection({ busy: true });

    expect(screen.getByText("正在检查新版本")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();
  });

  it("reports an up-to-date result after a completed empty check", () => {
    renderSection({ checkedAt: "2026-08-31T08:00:00Z" });

    expect(screen.getByText("已是最新版本")).toBeInTheDocument();
    expect(screen.getByText(/检查于/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "下载并安装" })).not.toBeInTheDocument();
  });

  it("offers download and installation for a signed update", async () => {
    const user = userEvent.setup();
    const props = renderSection({ result: available, checkedAt: available.checkedAt });

    expect(screen.getByText("发现新版本 0.2.0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "下载并安装" }));
    expect(props.onInstall).toHaveBeenCalledTimes(1);
  });

  it("shows byte progress while an update package has no content length", () => {
    renderSection({
      result: available,
      installing: true,
      progress: { downloadedBytes: 2048, totalBytes: null },
    });

    expect(screen.getByText("正在下载更新 2 KB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "下载并安装" })).toBeDisabled();
  });

  it("requires an explicit restart after a completed installation", async () => {
    const user = userEvent.setup();
    const props = renderSection({ restartRequired: true });

    expect(screen.getByText("更新已安装，需要重启")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "重新启动" }));
    expect(props.onRestart).toHaveBeenCalledTimes(1);
  });
});
