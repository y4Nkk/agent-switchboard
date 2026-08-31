import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProbePanel } from "./ProbePanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const reachable = {
  grade: "ok",
  status: 204,
  latencyMs: 320,
  error: null,
  at: "2026-08-31T05:00:00Z",
} as const;

describe("ProbePanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders nothing until the editor has an endpoint url", () => {
    render(<ProbePanel url={null} />);
    expect(screen.queryByRole("button", { name: "检测连通" })).not.toBeInTheDocument();
  });

  it("probes the current url and reports the reachable grade with latency", async () => {
    invokeMock.mockResolvedValueOnce(reachable);
    const user = userEvent.setup();
    render(<ProbePanel url="https://relay.example/v1" />);

    await user.click(screen.getByRole("button", { name: "检测连通" }));

    expect(invokeMock).toHaveBeenCalledWith("probe_endpoint", { url: "https://relay.example/v1" });
    expect(await screen.findByText(/连通正常 · HTTP 204 · 320 毫秒/)).toBeInTheDocument();
    expect(
      screen.getByText("检测仅确认服务地址可达，不发送模型请求，也不验证密钥是否有效。"),
    ).toBeInTheDocument();
  });

  it("grades a slow-but-reachable endpoint as a warning without failing it", async () => {
    invokeMock.mockResolvedValueOnce({ ...reachable, grade: "slow", latencyMs: 7400 });
    const user = userEvent.setup();
    render(<ProbePanel url="https://relay.example/v1" />);

    await user.click(screen.getByRole("button", { name: "检测连通" }));

    expect(await screen.findByText(/连通但较慢 · HTTP 204 · 7400 毫秒/)).toBeInTheDocument();
  });

  it("reports the failure class for unreachable endpoints", async () => {
    invokeMock.mockResolvedValueOnce({
      grade: "unreachable",
      status: null,
      latencyMs: 10_012,
      error: "连接超时",
      at: "2026-08-31T05:00:00Z",
    });
    const user = userEvent.setup();
    render(<ProbePanel url="https://relay.example/v1" />);

    await user.click(screen.getByRole("button", { name: "检测连通" }));

    expect(await screen.findByText(/无法连通 · 连接超时/)).toBeInTheDocument();
  });

  it("surfaces command failures as an alert and clears them on the next run", async () => {
    invokeMock
      .mockRejectedValueOnce(new Error("端点必须是 http(s) URL"))
      .mockResolvedValueOnce(reachable);
    const user = userEvent.setup();
    render(<ProbePanel url="ftp://relay.example" />);

    await user.click(screen.getByRole("button", { name: "检测连通" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("端点必须是 http(s) URL");

    await user.click(screen.getByRole("button", { name: "检测连通" }));
    await waitFor(() =>
      expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
    );
    expect(screen.getByText(/连通正常/)).toBeInTheDocument();
  });

  it("resets the displayed result when the url changes", () => {
    const { rerender } = render(<ProbePanel url="https://a.example" />);
    rerender(<ProbePanel url="https://b.example" />);
    expect(screen.queryByText(/连通正常/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检测连通" })).toBeEnabled();
  });
});
