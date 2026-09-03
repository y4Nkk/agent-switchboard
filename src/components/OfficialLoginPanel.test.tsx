import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { OfficialLoginPanel } from "./OfficialLoginPanel";

vi.mock("../api/client", () => ({
  startOfficialLogin: vi.fn(),
  pollOfficialLogin: vi.fn(),
  cancelOfficialLogin: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import {
  cancelOfficialLogin,
  pollOfficialLogin,
  startOfficialLogin,
} from "../api/client";
import { openUrl } from "@tauri-apps/plugin-opener";

const startMock = vi.mocked(startOfficialLogin);
const pollMock = vi.mocked(pollOfficialLogin);
const cancelMock = vi.mocked(cancelOfficialLogin);
const openMock = vi.mocked(openUrl);

const CODEX_START = {
  userCode: "CODE-1234",
  verificationUrl: "https://auth.openai.com/codex/device",
};

function codexStatus(phase: "pending" | "completed" | "failed") {
  return {
    phase,
    userCode: phase === "pending" ? "CODE-1234" : null,
    verificationUrl: phase === "pending" ? CODEX_START.verificationUrl : "",
    message: phase === "failed" ? "登录已超时，请重新开始" : null,
  };
}

describe("OfficialLoginPanel", () => {
  beforeEach(() => {
    startMock.mockReset();
    pollMock.mockReset();
    cancelMock.mockReset();
    openMock.mockReset();
    cancelMock.mockResolvedValue(undefined);
  });

  it("shows the Codex device code without opening the browser automatically", async () => {
    startMock.mockResolvedValue(CODEX_START);
    pollMock.mockResolvedValue(codexStatus("pending"));

    render(<OfficialLoginPanel app="codex" onFinished={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "开始官方登录" }));
    await act(async () => {});

    expect(screen.getByText("CODE-1234")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "打开验证页面" })).toBeInTheDocument();
    expect(openMock).not.toHaveBeenCalled();
    expect(startMock).toHaveBeenCalledWith("codex");
  });

  it("polls until completion and reports the finished login", async () => {
    startMock.mockResolvedValue(CODEX_START);
    pollMock.mockResolvedValueOnce(codexStatus("pending"));
    const onFinished = vi.fn();

    vi.useFakeTimers();
    try {
      render(<OfficialLoginPanel app="codex" onFinished={onFinished} />);
      fireEvent.click(screen.getByRole("button", { name: "开始官方登录" }));
      await act(async () => {});
      expect(pollMock).not.toHaveBeenCalled();

      await act(async () => {
        vi.advanceTimersByTime(3000);
      });
      expect(pollMock).toHaveBeenCalledTimes(1);
      expect(onFinished).not.toHaveBeenCalled();

      pollMock.mockResolvedValue(codexStatus("completed"));
      await act(async () => {
        vi.advanceTimersByTime(3000);
      });

      expect(
        screen.getByText("登录完成，登录凭据已写入客户端本地文件。"),
      ).toBeInTheDocument();
      expect(onFinished).toHaveBeenCalledWith(true);
      expect(screen.queryByRole("button", { name: "取消登录" })).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens the authorize page automatically for the Claude flow", async () => {
    startMock.mockResolvedValue({
      userCode: null,
      verificationUrl: "https://claude.ai/oauth/authorize?response_type=code",
    });
    pollMock.mockResolvedValue({
      phase: "pending",
      userCode: null,
      verificationUrl: "https://claude.ai/oauth/authorize?response_type=code",
      message: null,
    });

    render(<OfficialLoginPanel app="claude" onFinished={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "开始官方登录" }));
    await act(async () => {});

    expect(openMock).toHaveBeenCalledWith(
      "https://claude.ai/oauth/authorize?response_type=code",
    );
    expect(
      screen.getByText("已打开浏览器授权页面，请在该页面完成登录。"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/验证码/)).toBeNull();
  });

  it("renders a failed login with its message and reports the failure", async () => {
    startMock.mockRejectedValue(new Error("登录服务响应无法识别"));
    const onFinished = vi.fn();

    render(<OfficialLoginPanel app="codex" onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "开始官方登录" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("登录服务响应无法识别");
    expect(onFinished).toHaveBeenCalledWith(false);
  });

  it("survives a dropped poll and cancels back to idle on demand", async () => {
    startMock.mockResolvedValue(CODEX_START);
    pollMock.mockRejectedValue(new Error("后台任务已中断"));

    vi.useFakeTimers();
    try {
      render(<OfficialLoginPanel app="codex" onFinished={vi.fn()} />);
      fireEvent.click(screen.getByRole("button", { name: "开始官方登录" }));
      await act(async () => {});
      await act(async () => {
        vi.advanceTimersByTime(3000);
      });

      expect(screen.getByRole("alert")).toHaveTextContent("后台任务已中断");
      expect(screen.getByRole("button", { name: "取消登录" })).toBeInTheDocument();

      fireEvent.click(screen.getByRole("button", { name: "取消登录" }));
      await act(async () => {});

      expect(cancelMock).toHaveBeenCalledWith("codex");
      expect(
        screen.getByRole("button", { name: "开始官方登录" }),
      ).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });
});
