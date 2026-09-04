import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { RuntimeOverviewPanel } from "./RuntimeOverviewPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const overview = {
  appVersion: "0.1.5",
  buildMode: "release" as const,
  platform: "windows",
  architecture: "x86_64",
  transport: { kind: "webDevelopment" as const, host: "127.0.0.1", port: 1422, healthStatus: 204 },
  appDataPath: "C:/Users/test/AppData/Roaming/Agent Switchboard",
};

describe("RuntimeOverviewPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders the read-only application runtime facts on mount", async () => {
    invokeMock.mockResolvedValue(overview);
    render(<RuntimeOverviewPanel />);

    expect(await screen.findByText("v0.1.5")).toBeInTheDocument();
    expect(screen.getByText("正式构建")).toBeInTheDocument();
    expect(screen.getByText("Windows · x64")).toBeInTheDocument();
    expect(screen.getByText("127.0.0.1:1422")).toBeInTheDocument();
    expect(screen.getByText("健康检查 HTTP 204")).toBeInTheDocument();
    expect(screen.getByText(overview.appDataPath)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("runtime_overview");
  });

  it("labels the desktop shell as a non-TCP transport", async () => {
    invokeMock.mockResolvedValue({ ...overview, transport: { kind: "desktopProtocol" } });
    render(<RuntimeOverviewPanel />);

    expect(await screen.findByText("无 TCP 端口")).toBeInTheDocument();
    expect(screen.getByText("桌面协议 · 已响应")).toBeInTheDocument();
  });

  it("reports a failed runtime read without rendering stale details", async () => {
    invokeMock.mockRejectedValue(new Error("目录不可用"));
    render(<RuntimeOverviewPanel />);

    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取运行环境：目录不可用");
    expect(screen.queryByText("应用版本")).not.toBeInTheDocument();
  });
});
