import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CloudBackupPanel } from "./CloudBackupPanel";
import { getCloudBackupSetupSql, type CloudBackupSettings } from "../api/client";
import { openUrl } from "@tauri-apps/plugin-opener";

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return { ...actual, getCloudBackupSetupSql: vi.fn() };
});
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

const settings: CloudBackupSettings = {
  projectUrl: "https://example.supabase.co",
  publishableKey: "sb_publishable_example",
  email: "backup@example.com",
};

const originalClipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
const setupSqlMock = vi.mocked(getCloudBackupSetupSql);

beforeEach(() => {
  setupSqlMock.mockReset();
  vi.mocked(openUrl).mockReset();
});

afterEach(() => {
  if (originalClipboard) {
    Object.defineProperty(navigator, "clipboard", originalClipboard);
  } else {
    Reflect.deleteProperty(navigator, "clipboard");
  }
});

describe("CloudBackupPanel", () => {
  it("guides first-time Supabase setup with the required project-only credentials", async () => {
    const user = userEvent.setup();
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onTestConnection={vi.fn().mockResolvedValue(true)}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    const guide = screen.getByRole("region", { name: "从零配置 Supabase" });
    expect(within(guide).getAllByRole("listitem")).toHaveLength(6);
    expect(guide).toHaveTextContent("Project URL");
    expect(guide).toHaveTextContent("Publishable key");
    expect(guide).toHaveTextContent("不要使用 Account 的 Access Token");
    expect(guide).toHaveTextContent("Integrations → Data API");
    expect(guide).toHaveTextContent("Enable Data API");
    expect(guide).toHaveTextContent("Authentication → Users");
    expect(guide).toHaveTextContent("Add user → Create new user");
    expect(guide).toHaveTextContent("Auto Confirm User");
    expect(guide).toHaveTextContent("Dashboard 或 GitHub 的原有登录密码不能使用");
    expect(guide).toHaveTextContent("成功后会在当前窗口保留项目 Auth 密码");
    expect(guide).not.toHaveTextContent("成功后会清空项目 Auth 密码");
    expect(guide).toHaveTextContent("恢复必须使用同一条密码");
    expect(screen.getByText(/完整的供应商档案（包括端点、模型和 API 密钥）/)).toBeInTheDocument();
    expect(guide).toHaveTextContent("完整的供应商档案、通用配置和切换记录");
    expect(within(guide).getByRole("link", { name: "项目 Dashboard" })).toHaveAttribute(
      "href",
      "https://supabase.com/dashboard/project/example",
    );
    expect(within(guide).getByRole("link", { name: "Integrations → Data API" })).toHaveAttribute(
      "href",
      "https://supabase.com/dashboard/project/example/integrations/data_api/overview",
    );
    expect(within(guide).getByRole("link", { name: "SQL Editor" })).toHaveAttribute(
      "href",
      "https://supabase.com/dashboard/project/example/sql/new",
    );
    const authUsers = within(guide).getByRole("link", { name: "Authentication → Users" });
    expect(authUsers).toHaveAttribute(
      "href",
      "https://supabase.com/dashboard/project/example/auth/users",
    );
    await user.click(authUsers);
    expect(openUrl).toHaveBeenCalledWith(
      "https://supabase.com/dashboard/project/example/auth/users",
    );
  });

  it("copies the displayed initialization SQL", async () => {
    const user = userEvent.setup();
    const setupSql = "create table agent_switchboard_cloud_backups ();";
    const writeText = vi.fn().mockResolvedValue(undefined);
    setupSqlMock.mockResolvedValue(setupSql);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onTestConnection={vi.fn().mockResolvedValue(true)}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.click(screen.getByRole("button", { name: "显示初始化 SQL" }));
    expect(await screen.findByDisplayValue(setupSql)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "复制初始化 SQL" }));

    expect(writeText).toHaveBeenCalledWith(setupSql);
  });

  it("saves only connection coordinates", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn().mockResolvedValue(true);
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={onSave}
        onTestConnection={vi.fn().mockResolvedValue(true)}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.clear(screen.getByLabelText("项目 Auth 登录邮箱"));
    await user.type(screen.getByLabelText("项目 Auth 登录邮箱"), "new@example.com");
    await user.click(screen.getByRole("button", { name: "保存连接" }));

    expect(onSave).toHaveBeenCalledWith({ ...settings, email: "new@example.com" });
  });

  it("tests the current unsaved connection draft without discarding the password", async () => {
    const user = userEvent.setup();
    const onTestConnection = vi.fn().mockResolvedValue(true);
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onTestConnection={onTestConnection}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.clear(screen.getByLabelText("项目 Auth 登录邮箱"));
    await user.type(screen.getByLabelText("项目 Auth 登录邮箱"), "new@example.com");
    await user.type(screen.getByLabelText("项目 Auth 登录密码"), "account-password");
    await user.click(screen.getByRole("button", { name: "测试连接" }));

    await waitFor(() =>
      expect(onTestConnection).toHaveBeenCalledWith(
        { ...settings, email: "new@example.com" },
        "account-password",
      ),
    );
    expect(screen.getByLabelText("项目 Auth 登录密码")).toHaveValue("account-password");
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
        onTestConnection={vi.fn().mockResolvedValue(true)}
        onUpload={onUpload}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    await user.type(screen.getByLabelText("项目 Auth 登录密码"), "account-password");
    await user.type(screen.getByLabelText("备份密码（自行设置）"), "backup-password");
    await user.click(screen.getByRole("button", { name: "备份到云端" }));

    expect(onUpload).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认备份" }));
    await waitFor(() =>
      expect(onUpload).toHaveBeenCalledWith("account-password", "backup-password"),
    );
    expect(screen.getByLabelText("项目 Auth 登录密码")).toHaveValue("");
    expect(screen.getByLabelText("备份密码（自行设置）")).toHaveValue("");
  });

  it("uses a destructive confirmation before replacing local profile data", async () => {
    const user = userEvent.setup();
    render(
      <CloudBackupPanel
        settings={settings}
        loaded
        busy={false}
        onSave={vi.fn().mockResolvedValue(true)}
        onTestConnection={vi.fn().mockResolvedValue(true)}
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
        onTestConnection={vi.fn().mockResolvedValue(true)}
        onUpload={vi.fn().mockResolvedValue(true)}
        onRestore={vi.fn().mockResolvedValue(true)}
      />,
    );

    expect(screen.getByRole("button", { name: "备份到云端" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "从云端恢复" })).toBeDisabled();
  });
});
