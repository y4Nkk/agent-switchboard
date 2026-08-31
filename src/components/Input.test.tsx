import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Input } from "./Input";

function Harness({ code = false, disabled = false }: { code?: boolean; disabled?: boolean }) {
  const [value, setValue] = useState("");
  return (
    <Input
      aria-label="服务地址"
      code={code}
      disabled={disabled}
      placeholder="https://example.com"
      value={value}
      onChange={(event) => setValue(event.target.value)}
    />
  );
}

describe("Input", () => {
  it("passes native attributes through to the accessibility tree", () => {
    render(<Harness />);
    expect(screen.getByRole("textbox", { name: "服务地址" })).toHaveAttribute(
      "placeholder",
      "https://example.com",
    );
  });

  it("emits typed text through onChange", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(screen.getByRole("textbox"), "abc");
    expect(screen.getByRole("textbox")).toHaveValue("abc");
  });

  it("stays inert when disabled", async () => {
    const user = userEvent.setup();
    render(<Harness disabled />);

    const field = screen.getByRole("textbox");
    expect(field).toBeDisabled();
    await user.type(field, "abc");
    expect(field).toHaveValue("");
  });

  it("applies the monospace code variant", () => {
    render(<Harness code />);
    expect(screen.getByRole("textbox")).toHaveClass("asb-code");
  });
});
