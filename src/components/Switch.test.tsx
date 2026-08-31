import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Switch } from "./Switch";

describe("Switch", () => {
  it("exposes its label as the accessible switch name without drawing it", () => {
    render(<Switch checked={false} label="启用硬件加速" onChange={() => {}} />);

    expect(screen.getByRole("switch", { name: "启用硬件加速" })).toBeInTheDocument();
    expect(screen.queryByText("启用硬件加速")).toBeNull();
  });

  it("reflects the checked state on the drawn track", () => {
    render(<Switch checked label="窗口始终置顶" onChange={() => {}} />);

    expect(screen.getByRole("switch", { name: "窗口始终置顶" })).toBeChecked();
    expect(document.querySelector(".asb-switch-track")?.getAttribute("data-checked")).toBe("true");
  });

  it("emits the next state on click", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Switch checked={false} label="窗口始终置顶" onChange={onChange} />);

    await user.click(screen.getByRole("switch", { name: "窗口始终置顶" }));

    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("blocks interaction while disabled", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Switch checked label="启用硬件加速" disabled onChange={onChange} />);

    const control = screen.getByRole("switch", { name: "启用硬件加速" });
    expect(control).toBeDisabled();
    await user.click(control);
    expect(onChange).not.toHaveBeenCalled();
  });
});
