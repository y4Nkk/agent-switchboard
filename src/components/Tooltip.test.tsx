import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Tooltip } from "./Tooltip";

describe("Tooltip", () => {
  it("keeps the trigger interactive and hides the label while closed", () => {
    render(
      <Tooltip label="先查看变更并确认候选内容，才能安全切换">
        <button type="button">安全切换</button>
      </Tooltip>,
    );

    expect(screen.getByRole("button", { name: "安全切换" })).toBeInTheDocument();
    expect(screen.queryByText(/确认候选内容/)).not.toBeInTheDocument();
  });

  it("opens on hover and closes again", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <Tooltip
          label="默认：不写入该行，由 Codex 内置默认值决定"
          open={open}
          onOpenChange={setOpen}
        >
          <span tabIndex={0}>高</span>
        </Tooltip>
      );
    }
    render(<Harness />);

    await user.hover(screen.getByText("高"));
    expect(await screen.findByText(/不写入该行/)).toBeInTheDocument();

    await user.unhover(screen.getByText("高"));
    await waitFor(() => {
      expect(screen.queryByText(/不写入该行/)).not.toBeInTheDocument();
    });
  });
});
