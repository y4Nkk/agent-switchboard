import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GeneralSettingsForm } from "./GeneralSettingsForm";
import type { CommonConfigPatch } from "../api/client";

const codexPatch: CommonConfigPatch = {
  app: "codex",
  entries: [{ key: "disable_response_storage", value: false }],
};

describe("GeneralSettingsForm", () => {
  it("uses a Chinese label for the setting", () => {
    render(<GeneralSettingsForm patch={codexPatch} busy={false} onChange={() => {}} />);
    expect(screen.getByLabelText("禁用响应存储")).toBeInTheDocument();
  });

  it("emits a typed patch, not configuration text", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<GeneralSettingsForm patch={codexPatch} busy={false} onChange={onChange} />);

    await user.click(screen.getByLabelText("禁用响应存储"));
    expect(onChange).toHaveBeenCalledWith({
      app: "codex",
      entries: [{ key: "disable_response_storage", value: true }],
    });
  });

  it("states that Claude model routing lives in provider profiles", () => {
    render(
      <GeneralSettingsForm patch={{ app: "claude", entries: [] }} busy={false} onChange={() => {}} />,
    );
    expect(screen.getByText(/供应商档案/)).toBeInTheDocument();
  });
});
