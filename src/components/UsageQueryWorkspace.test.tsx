import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { UsageQuery } from "../api/client";
import { UsageQueryWorkspace } from "./UsageQueryWorkspace";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

function renderWorkspace(initial: UsageQuery | null = null, onSave = vi.fn(async () => true)) {
  render(
    <UsageQueryWorkspace
      providerName="测试中转"
      value={initial}
      apiKey="sk-live"
      baseUrl="https://relay.example/v1"
      busy={false}
      onSave={onSave}
      onClose={() => {}}
    />,
  );
  return onSave;
}

describe("UsageQueryWorkspace", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("uses a full workspace with two optional query modes", async () => {
    const user = userEvent.setup();
    renderWorkspace();

    expect(screen.getByRole("region", { name: "用量查询" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "字段提取" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    await user.click(screen.getByRole("tab", { name: "自编脚本" }));
    expect(screen.getByRole("textbox", { name: "用量查询脚本" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "自编脚本" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("runs a self-authored script with the current draft credential", async () => {
    invokeMock.mockResolvedValueOnce({
      remaining: 9.5,
      used: 0.5,
      total: 10,
      unit: "USD",
      at: "2026-08-31T07:00:00Z",
    });
    const user = userEvent.setup();
    const onSave = renderWorkspace();

    await user.click(screen.getByRole("tab", { name: "自编脚本" }));
    fireEvent.change(screen.getByRole("textbox", { name: "用量查询脚本" }), {
      target: {
        value:
          "({ request: () => ({ url: 'https://relay.example/balance', method: 'GET' }), extract: () => ({ remaining: 9.5, unit: 'USD' }) })",
      },
    });
    await user.click(screen.getByRole("button", { name: "查询用量" }));

    expect(invokeMock).toHaveBeenCalledWith("test_usage_query", {
      query: expect.objectContaining({ kind: "script" }),
      apiKey: "sk-live",
      baseUrl: "https://relay.example/v1",
    });
    expect(await screen.findByRole("region", { name: "本次用量结果" })).toHaveTextContent(
      "9.5",
    );

    await user.click(screen.getByRole("button", { name: "保存查询" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: "script",
        source:
          "({ request: () => ({ url: 'https://relay.example/balance', method: 'GET' }), extract: () => ({ remaining: 9.5, unit: 'USD' }) })",
      }),
    );
  });

  it("normalizes and saves the declarative draft independently", async () => {
    const user = userEvent.setup();
    const onSave = renderWorkspace();

    fireEvent.change(screen.getByLabelText("用量查询地址"), {
      target: { value: "{{baseUrl}}/user/balance " },
    });
    fireEvent.change(screen.getByLabelText("余额提取路径"), {
      target: { value: "data/balance " },
    });
    fireEvent.change(screen.getByLabelText("用量单位"), { target: { value: "USD" } });
    await user.click(screen.getByRole("button", { name: "保存查询" }));

    expect(onSave).toHaveBeenCalledWith({
      kind: "declarative",
      url: "{{baseUrl}}/user/balance",
      remainingPath: "data/balance",
      usedPath: null,
      totalPath: null,
      unit: "USD",
    });
  });

  it("clears an optional declarative query when its fields are saved blank", async () => {
    const user = userEvent.setup();
    invokeMock.mockResolvedValue({
      remaining: 4,
      used: null,
      total: null,
      unit: "USD",
      at: "2026-08-31T08:00:00Z",
    });
    const onSave = renderWorkspace({
      kind: "declarative",
      url: "{{baseUrl}}/balance",
      remainingPath: "data/balance",
    });

    await screen.findByRole("region", { name: "本次用量结果" });
    fireEvent.change(screen.getByLabelText("用量查询地址"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("余额提取路径"), { target: { value: "" } });
    await user.click(screen.getByRole("button", { name: "保存查询" }));

    expect(onSave).toHaveBeenCalledWith(null);
  });

  it("shows a returned error without rendering the credential", async () => {
    invokeMock.mockRejectedValueOnce(new Error("用量查询返回 HTTP 402"));
    const user = userEvent.setup();
    renderWorkspace({
      kind: "declarative",
      url: "{{baseUrl}}/balance",
      remainingPath: "balance",
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("用量查询返回 HTTP 402");
    expect(screen.queryByText(/sk-live/)).not.toBeInTheDocument();

    // The configured workspace auto-runs on entry exactly once.
    expect(invokeMock).toHaveBeenCalledTimes(1);
    await user.keyboard("{Escape}");
  });
});
