import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { PinTopButton } from "./PinTopButton";

describe("PinTopButton", () => {
  it("未置顶态可点，提示与可及名为置顶窗口", () => {
    const onToggle = vi.fn();
    render(<PinTopButton active={false} disabled={false} onToggle={onToggle} />);
    const button = screen.getByRole("button", { name: "置顶窗口" });
    expect(button.getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(button);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("置顶态镜像为取消置顶并携带 aria-pressed", () => {
    const onToggle = vi.fn();
    render(<PinTopButton active={true} disabled={false} onToggle={onToggle} />);
    const button = screen.getByRole("button", { name: "取消置顶" });
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(screen.queryByRole("button", { name: "置顶窗口" })).toBeNull();
  });

  it("禁用态点击不分发切换", () => {
    const onToggle = vi.fn();
    render(<PinTopButton active={false} disabled={true} onToggle={onToggle} />);
    fireEvent.click(screen.getByRole("button", { name: "置顶窗口" }));
    expect(onToggle).not.toHaveBeenCalled();
  });
});
