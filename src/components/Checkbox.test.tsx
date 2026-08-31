import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Checkbox } from "./Checkbox";

function Harness({ onChange }: { onChange?: (checked: boolean) => void }) {
  const [checked, setChecked] = useState(false);
  return (
    <Checkbox
      label="禁用响应存储"
      checked={checked}
      disabled={false}
      onChange={(next) => {
        setChecked(next);
        onChange?.(next);
      }}
    />
  );
}

describe("Checkbox", () => {
  it("uses its visible label as the accessible name", () => {
    render(<Checkbox checked={false} label="禁用响应存储" onChange={() => {}} />);
    expect(screen.getByRole("checkbox", { name: "禁用响应存储" })).toBeInTheDocument();
  });

  it("emits the next checked state on click and reflects it back", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);

    await user.click(screen.getByRole("checkbox"));
    expect(onChange).toHaveBeenCalledWith(true);
    expect(screen.getByRole("checkbox")).toBeChecked();

    await user.click(screen.getByRole("checkbox"));
    expect(onChange).toHaveBeenLastCalledWith(false);
    expect(screen.getByRole("checkbox")).not.toBeChecked();
  });

  it("toggles with the keyboard", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.tab();
    expect(screen.getByRole("checkbox")).toHaveFocus();

    await user.keyboard(" ");
    expect(screen.getByRole("checkbox")).toBeChecked();
  });

  it("ignores interaction when disabled", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Checkbox checked={true} label="禁用响应存储" disabled onChange={onChange} />);

    await user.click(screen.getByRole("checkbox"));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("checkbox")).toBeDisabled();
  });

  it("announces indeterminate as mixed and still toggles on click", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Checkbox checked={false} label="禁用响应存储" indeterminate onChange={onChange} />);

    expect(screen.getByRole("checkbox")).toHaveAttribute("aria-checked", "mixed");
    await user.click(screen.getByRole("checkbox"));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
