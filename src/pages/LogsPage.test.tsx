import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  listRuntimeLogs,
  openRuntimeLogDir,
  type RuntimeLogEntry,
  type RuntimeLogLevel,
} from "../api/client";
import { LogsPage } from "./LogsPage";

vi.mock("../api/client", () => ({
  listRuntimeLogs: vi.fn(),
  openRuntimeLogDir: vi.fn(),
}));

const listRuntimeLogsMock = vi.mocked(listRuntimeLogs);
const openRuntimeLogDirMock = vi.mocked(openRuntimeLogDir);

const entries: RuntimeLogEntry[] = [
  {
    at: "2026-08-26T10:00:00Z",
    level: "error",
    action: "configurationSwitched",
    errorCode: "preview-stale",
  },
  {
    at: "2026-08-26T09:00:00Z",
    level: "warn",
    action: "profileStoreReset",
  },
  {
    at: "2026-08-26T08:00:00Z",
    level: "info",
    action: "appStarted",
  },
  {
    at: "2026-08-26T07:00:00Z",
    level: "debug",
    action: "profileUpdated",
  },
];

function renderLogsPage({
  logLevel = "info",
  busy = false,
  onLogLevelChange = vi.fn(),
}: {
  logLevel?: RuntimeLogLevel | null;
  busy?: boolean;
  onLogLevelChange?: (level: RuntimeLogLevel) => void;
} = {}) {
  render(<LogsPage logLevel={logLevel} busy={busy} onLogLevelChange={onLogLevelChange} />);
  return { onLogLevelChange };
}

describe("LogsPage", () => {
  beforeEach(() => {
    listRuntimeLogsMock.mockReset();
    openRuntimeLogDirMock.mockReset();
  });

  it("reads application runtime events and preserves backend order", async () => {
    listRuntimeLogsMock.mockResolvedValue(entries);
    renderLogsPage();

    expect(await screen.findByText("已切换配置")).toBeInTheDocument();
    const rows = screen.getAllByRole("row");
    expect(rows).toHaveLength(5);
    expect(within(rows[1]).getByText("错误")).toBeInTheDocument();
    expect(within(rows[1]).getByText("preview-stale")).toBeInTheDocument();
    expect(within(rows[3]).getByText("应用已启动")).toBeInTheDocument();
    expect(within(rows[4]).getByText("调试")).toBeInTheDocument();
    expect(listRuntimeLogsMock).toHaveBeenCalledTimes(1);
  });

  it("filters by level and reloads through the same typed reader", async () => {
    listRuntimeLogsMock.mockResolvedValue(entries);
    const user = userEvent.setup();
    renderLogsPage();

    await screen.findByText("已切换配置");
    await user.click(screen.getByRole("button", { name: "警告" }));
    expect(screen.queryByText("应用已启动")).not.toBeInTheDocument();
    expect(screen.queryByText("已切换配置")).not.toBeInTheDocument();
    expect(screen.getByText("已重置供应商数据")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "刷新" }));
    expect(listRuntimeLogsMock).toHaveBeenCalledTimes(2);
  });

  it("uses the complete Chinese recording-level menu and emits its selected threshold", async () => {
    listRuntimeLogsMock.mockResolvedValue([]);
    const user = userEvent.setup();
    const { onLogLevelChange } = renderLogsPage();

    await screen.findByText("暂无应用运行日志");
    await user.click(screen.getByRole("combobox", { name: "记录级别" }));
    expect(screen.getByRole("option", { name: "调试" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "信息" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "警告" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "错误" })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: "静默" }));

    expect(onLogLevelChange).toHaveBeenCalledWith("silent");
  });

  it("states when no runtime event has been recorded", async () => {
    listRuntimeLogsMock.mockResolvedValue([]);
    renderLogsPage();

    expect(await screen.findByText("暂无应用运行日志")).toBeInTheDocument();
  });

  it("opens only the app-owned runtime-log directory through the typed command", async () => {
    listRuntimeLogsMock.mockResolvedValue([]);
    openRuntimeLogDirMock.mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderLogsPage();

    await screen.findByText("暂无应用运行日志");
    await user.click(screen.getByRole("button", { name: "打开日志文件夹" }));

    expect(openRuntimeLogDirMock).toHaveBeenCalledTimes(1);
  });

  it("reports a folder-open error without hiding the current logs", async () => {
    listRuntimeLogsMock.mockResolvedValue(entries);
    openRuntimeLogDirMock.mockRejectedValue({
      code: "runtime-log-directory-open-failed",
      message: "无法打开应用日志文件夹",
    });
    const user = userEvent.setup();
    renderLogsPage();

    await screen.findByText("已切换配置");
    await user.click(screen.getByRole("button", { name: "打开日志文件夹" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("无法打开应用日志文件夹");
    expect(screen.getByText("已切换配置")).toBeInTheDocument();
  });

  it("keeps an existing table visible when a manual refresh fails", async () => {
    listRuntimeLogsMock.mockResolvedValueOnce(entries).mockRejectedValueOnce({
      code: "runtime-log-unavailable",
      message: "应用日志不可读",
    });
    const user = userEvent.setup();
    renderLogsPage();

    await screen.findByText("已切换配置");
    await user.click(screen.getByRole("button", { name: "刷新" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("应用日志不可读");
    expect(screen.getByText("已切换配置")).toBeInTheDocument();
  });
});
