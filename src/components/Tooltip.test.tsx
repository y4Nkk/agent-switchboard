import { useState } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
    render(
      <Tooltip label="默认：不写入该行，由 Codex 内置默认值决定">
        <span tabIndex={0}>高</span>
      </Tooltip>,
    );

    await user.hover(screen.getByText("高"));
    expect(await screen.findByText(/不写入该行/)).toBeInTheDocument();

    await user.unhover(screen.getByText("高"));
    await waitFor(() => {
      expect(screen.queryByText(/不写入该行/)).not.toBeInTheDocument();
    });
  });

  it("dismisses after an action until the pointer leaves the trigger", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [completed, setCompleted] = useState(false);
      return (
        <>
          <Tooltip label="执行此操作">
            <button type="button" onClick={() => setCompleted(true)}>
              执行
            </button>
          </Tooltip>
          <output>{completed ? "已执行" : "未执行"}</output>
        </>
      );
    }
    render(<Harness />);
    const action = screen.getByRole("button", { name: "执行" });

    await user.hover(action);
    expect(await screen.findByText("执行此操作")).toBeInTheDocument();

    await user.click(action);
    expect(screen.getByText("已执行")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText("执行此操作")).not.toBeInTheDocument();
    });

    fireEvent.pointerMove(action, { pointerType: "mouse" });
    expect(screen.queryByText("执行此操作")).not.toBeInTheDocument();

    await user.unhover(action);
    await user.hover(action);
    expect(await screen.findByText("执行此操作")).toBeInTheDocument();
  });

  it("dismisses after keyboard activation", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [completed, setCompleted] = useState(false);
      return (
        <>
          <Tooltip label="通过键盘执行">
            <button type="button" onClick={() => setCompleted(true)}>
              执行
            </button>
          </Tooltip>
          <output>{completed ? "已执行" : "未执行"}</output>
        </>
      );
    }
    render(<Harness />);

    await user.tab();
    expect(await screen.findByText("通过键盘执行")).toBeInTheDocument();

    await user.keyboard("{Enter}");
    expect(screen.getByText("已执行")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText("通过键盘执行")).not.toBeInTheDocument();
    });
  });

  it("keeps an action dismissal across programmatic focus restore", async () => {
    const user = userEvent.setup();
    render(
      <Tooltip label="执行此操作">
        <button type="button">执行</button>
      </Tooltip>,
    );
    const action = screen.getByRole("button", { name: "执行" });

    await user.hover(action);
    expect(await screen.findByText("执行此操作")).toBeInTheDocument();
    await user.click(action);
    await waitFor(() => {
      expect(screen.queryByText("执行此操作")).not.toBeInTheDocument();
    });

    // A dialog stealing focus and handing it back restores focus without a
    // relatedTarget; the dismissal must survive that round trip.
    fireEvent.blur(action);
    fireEvent.focus(action);
    expect(screen.queryByText("执行此操作")).not.toBeInTheDocument();

    // Keyboard navigation back to the control lifts the dismissal. Real Tab
    // keystrokes carry a relatedTarget; jsdom's synthesized focus does not,
    // so the Tab shape is expressed through the event init here.
    fireEvent.focus(action, { relatedTarget: document.body });
    expect(await screen.findByText("执行此操作")).toBeInTheDocument();
  });

});
