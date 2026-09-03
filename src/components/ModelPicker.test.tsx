import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModelPicker } from "./ModelPicker";
import type { ProviderModel } from "../api/client";

const MODELS: ProviderModel[] = [
  { id: "deepseek-v4", ownedBy: "deepseek" },
  { id: "kimi-k3", ownedBy: null },
  { id: "claude-sonnet-4-6", ownedBy: "anthropic" },
  { id: "claude-haiku-4-5", ownedBy: "anthropic" },
];

function renderPicker(props: Partial<Parameters<typeof ModelPicker>[0]> = {}) {
  return render(
    <ModelPicker
      models={props.models ?? MODELS}
      current={props.current ?? null}
      ariaLabel={props.ariaLabel ?? "选择模型"}
      disabled={props.disabled}
      onSelect={props.onSelect ?? vi.fn()}
    />,
  );
}

describe("ModelPicker", () => {
  it("groups fetched models by vendor and marks the current one", async () => {
    const user = userEvent.setup();
    renderPicker({ current: "kimi-k3" });

    await user.click(screen.getByRole("button", { name: "选择模型" }));

    expect(screen.getByRole("group", { name: "anthropic" })).toBeDefined();
    expect(screen.getByRole("group", { name: "deepseek" })).toBeDefined();
    expect(screen.getByRole("group", { name: "其他" })).toBeDefined();
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "claude-haiku-4-5",
      "claude-sonnet-4-6",
      "deepseek-v4",
      "kimi-k3",
    ]);
    expect(screen.getByRole("option", { name: "kimi-k3" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("option", { name: "deepseek-v4" }).getAttribute("aria-selected")).toBe(
      "false",
    );
  });

  it("filters by model id and by vendor name, with an empty state", async () => {
    const user = userEvent.setup();
    renderPicker();

    await user.click(screen.getByRole("button", { name: "选择模型" }));
    const search = screen.getByLabelText("搜索选择模型");

    await user.type(search, "anth");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "claude-haiku-4-5",
      "claude-sonnet-4-6",
    ]);

    await user.clear(search);
    await user.type(search, "kimi");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["kimi-k3"]);

    await user.clear(search);
    await user.type(search, "x");
    expect(screen.queryByRole("option")).toBeNull();
    expect(screen.getByText("没有找到相关模型")).toBeInTheDocument();
  });

  it("emits the chosen model, closes the menu, and refocuses the trigger", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderPicker({ onSelect });

    const trigger = screen.getByRole("button", { name: "选择模型" });
    await user.click(trigger);
    await user.click(screen.getByRole("option", { name: "deepseek-v4" }));

    expect(onSelect).toHaveBeenCalledWith("deepseek-v4");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(trigger).toHaveFocus();
  });

  it("closes on Escape and on an outside pointer press", async () => {
    const user = userEvent.setup();
    render(
      <>
        <ModelPicker models={MODELS} current={null} ariaLabel="选择模型" onSelect={vi.fn()} />
        <button type="button">外部</button>
      </>,
    );

    await user.click(screen.getByRole("button", { name: "选择模型" }));
    expect(screen.getByRole("listbox")).toBeDefined();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();

    await user.click(screen.getByRole("button", { name: "选择模型" }));
    await user.click(screen.getByRole("button", { name: "外部" }));
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("moves focus with arrow keys inside the option list", async () => {
    const user = userEvent.setup();
    renderPicker();

    await user.click(screen.getByRole("button", { name: "选择模型" }));
    await user.type(screen.getByLabelText("搜索选择模型"), "{ArrowDown}");

    expect(screen.getByRole("option", { name: "claude-haiku-4-5" })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("option", { name: "claude-sonnet-4-6" })).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(screen.getByRole("option", { name: "claude-haiku-4-5" })).toHaveFocus();
  });

  it("disables the trigger while busy", () => {
    renderPicker({ disabled: true });

    expect(screen.getByRole("button", { name: "选择模型" })).toBeDisabled();
  });
});
