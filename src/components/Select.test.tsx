import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Select } from "./Select";

const options = [
  { value: "codex", label: "Codex" },
  { value: "claude", label: "Claude" },
];

describe("Select", () => {
  it("shows the chosen label, or the placeholder while no value is set", () => {
    const { rerender } = render(
      <Select ariaLabel="客户端" value="codex" options={options} onChange={() => {}} />,
    );
    expect(screen.getByRole("combobox", { name: "客户端" })).toHaveTextContent("Codex");

    rerender(
      <Select ariaLabel="客户端" value={null} options={options} placeholder="（未选择）" onChange={() => {}} />,
    );
    expect(screen.getByRole("combobox", { name: "客户端" })).toHaveTextContent("（未选择）");
  });

  it("opens on click and emits the chosen option value", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Select ariaLabel="客户端" value={null} options={options} onChange={onChange} />);

    await user.click(screen.getByRole("combobox", { name: "客户端" }));
    await user.click(await screen.findByRole("option", { name: "Claude" }));

    expect(onChange).toHaveBeenCalledWith("claude");
  });

  it("marks the current option with a check indicator", async () => {
    const user = userEvent.setup();
    render(<Select ariaLabel="客户端" value="claude" options={options} onChange={() => {}} />);

    await user.click(screen.getByRole("combobox", { name: "客户端" }));
    const indicator = await screen
      .findByRole("option", { name: "Claude" })
      .then((option) => option.querySelector(".asb-select-check svg"));
    expect(indicator).toBeInTheDocument();
  });

  it("does not open when disabled", async () => {
    const user = userEvent.setup();
    render(
      <Select ariaLabel="客户端" value="codex" options={options} onChange={() => {}} disabled />,
    );

    await user.click(screen.getByRole("combobox", { name: "客户端" }));
    expect(screen.queryByRole("option")).toBeNull();
  });
});
