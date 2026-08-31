import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Textarea } from "./Textarea";

function Harness({ code = false }: { code?: boolean }) {
  const [value, setValue] = useState("");
  return (
    <Textarea
      aria-label="备注"
      code={code}
      rows={2}
      value={value}
      onChange={(event) => setValue(event.target.value)}
    />
  );
}

describe("Textarea", () => {
  it("passes native attributes through to the accessibility tree", () => {
    render(<Harness />);
    expect(screen.getByRole("textbox", { name: "备注" })).toHaveAttribute("rows", "2");
  });

  it("accepts multi-line input", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(screen.getByRole("textbox"), "a{Enter}b");
    expect(screen.getByRole("textbox")).toHaveValue("a\nb");
  });

  it("applies the monospace code variant", () => {
    render(<Harness code />);
    expect(screen.getByRole("textbox")).toHaveClass("asb-code");
  });
});
