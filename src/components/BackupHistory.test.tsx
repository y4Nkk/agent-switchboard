import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BackupHistory } from "./BackupHistory";
import * as client from "../api/client";
import type { BackupRecord } from "../api/client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const records: BackupRecord[] = [
  {
    id: "abc123-20260826",
    app: "codex",
    targetPath: "C:/Users/test/.codex/config.toml",
    backupPath: "C:/Users/test/AppData/Roaming/Agent Switchboard/state/backups/config.toml.1.bak",
    createdAt: "2026-08-26T08:30:00.000+00:00",
    contentHash: "abcdef0123456789abcdef0123456789",
    targetExisted: true,
    reason: "switch",
  },
  {
    id: "prerestore-20260826",
    app: "claude",
    targetPath: "C:/Users/test/.claude/settings.json",
    backupPath: "C:/Users/test/AppData/Roaming/Agent Switchboard/state/backups/settings.json.2.prerestore.bak",
    createdAt: "2026-08-26T09:00:00.000+00:00",
    contentHash: "0123456789abcdef0123456789abcdef",
    targetExisted: true,
    reason: "restore-precheck",
  },
];

describe("BackupHistory", () => {
  it("lists records with time, client, reason and hash prefix", () => {
    render(<BackupHistory records={records} busy={false} onRestore={() => {}} />);
    expect(screen.getAllByRole("button", { name: "恢复" }).length).toBe(2);
    expect(screen.getByText(/abcdef012345/)).toBeInTheDocument();
    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(screen.getByText("恢复前备份", { exact: false })).toBeInTheDocument();
  });

  it("requires confirmation before restoring", async () => {
    const user = userEvent.setup();
    const onRestore = vi.fn();
    render(<BackupHistory records={records} busy={false} onRestore={onRestore} />);

    await user.click(screen.getAllByRole("button", { name: "恢复" })[0]);
    expect(onRestore).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "确认恢复" }));
    expect(onRestore).toHaveBeenCalledWith("abc123-20260826");
  });

  it("shows the owned-key difference between a backup and the live file", async () => {
    const diffSpy = vi.spyOn(client, "backupDiff").mockResolvedValue([
      { key: "model", kind: "set", before: "gpt-5.1", after: "gpt-5.2" },
    ]);
    const user = userEvent.setup();
    render(<BackupHistory records={records} busy={false} onRestore={() => {}} />);

    await user.click(screen.getAllByRole("button", { name: "差异" })[0]);
    await waitFor(() => expect(diffSpy).toHaveBeenCalledWith("abc123-20260826"));
    expect(await screen.findByText(/gpt-5\.1 → gpt-5\.2/)).toBeInTheDocument();
  });

  it("reports when a backup matches the live file", async () => {
    vi.spyOn(client, "backupDiff").mockResolvedValue([]);
    const user = userEvent.setup();
    render(<BackupHistory records={records} busy={false} onRestore={() => {}} />);

    await user.click(screen.getAllByRole("button", { name: "差异" })[1]);
    expect(await screen.findByText("与当前文件一致")).toBeInTheDocument();
  });

  it("shows an empty state when there is nothing to restore", () => {
    render(<BackupHistory records={[]} busy={false} onRestore={() => {}} />);
    expect(screen.getByText("暂无备份")).toBeInTheDocument();
  });
});
