import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CloudBackupPanel } from "./CloudBackupPanel";
import type { CloudBackupSettings } from "../api/client";

const settings: CloudBackupSettings = {
  projectUrl: "https://example.supabase.co",
  publishableKey: "sb_publishable_example",
  email: "backup@example.com",
};

describe("CloudBackupPanel", () => {
  it("saves only connection coordinates", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn().mockResolvedValue(true);
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={onSave}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.clear(screen.getByLabelText("Supabase 登录邮箱"));
    await user.type(screen.getByLabelText("Supabase 登录邮箱"), "new@example.com");
    await user.click(screen.getByRole("button", { name: "保存连接" }));

    expect(onSave).toHaveBeenCalledWith({ ...settings, email: "new@example.com" });
  });

  it("requires confirmation before replacing the remote backup", async () => {
    const user = userEvent.setup();
    const onUpload = vi.fn().mockResolvedValue(true);
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onUpload={onUpload}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.type(screen.getByLabelText("Supabase 登录密码"), "account-password");
    await user.type(screen.getByLabelText("备份密码"), "backup-password");
    await user.click(screen.getByRole("button", { name: "备份到云端" }));

    expect(onUpload).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认备份" }));
    await waitFor(() =>
      expect(onUpload).toHaveBeenCalledWith("account-password", "backup-password"),
    );
    expect(screen.getByLabelText("Supabase 登录密码")).toHaveValue("");
    expect(screen.getByLabelText("备份密码")).toHaveValue("");
  });

  it("uses a destructive confirmation before replacing local profile data", async () => {
    const user = userEvent.setup();
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.click(screen.getByRole("button", { name: "从云端恢复" }));

    expect(screen.getByRole("dialog", { name: "确认从云端恢复" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "确认恢复" })).toHaveClass("asb-btn-danger");
  });

  it("leaves cloud actions unavailable until connection settings are saved", () => {
    render(
      <CloudBackupPanel
        settings={null}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    expect(screen.getByRole("button", { name: "备份到云端" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "从云端恢复" })).toBeDisabled();
  });
});
