import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { Toaster } from "./Toaster";
import { toast } from "./use-toast";

// Store cleanup between tests is owned by the global setup (clearToasts).
function toastSurface() {
  return screen.getByRole("status").closest(".asb-toast");
}

describe("Toaster", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders title and description with the kind's role", () => {
    render(<Toaster />);
    act(() => {
      toast({ kind: "success", title: "已保存", description: "配置已写入" });
    });
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("已保存");
    expect(status).toHaveTextContent("配置已写入");

    act(() => {
      toast({ kind: "error", title: "切换失败" });
    });
    expect(screen.getByRole("alert")).toHaveTextContent("切换失败");
  });

  it("auto-dismisses after the kind's default duration", () => {
    render(<Toaster />);
    act(() => {
      toast({ kind: "success", title: "已保存" });
    });

    act(() => {
      vi.advanceTimersByTime(3199);
    });
    expect(screen.getByRole("status")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("pauses auto-dismiss while hovered and resumes on leave", () => {
    render(<Toaster />);
    act(() => {
      toast({ kind: "info", title: "已导入" });
    });

    // React synthesizes pointerenter/leave from over/out, as a real hover does.
    fireEvent.pointerOver(toastSurface()!);
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    expect(screen.getByRole("status")).toBeInTheDocument();

    fireEvent.pointerOut(toastSurface()!);
    act(() => {
      vi.advanceTimersByTime(4500);
    });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("auto-dismisses error toasts on the longer error delay and supports manual close", () => {
    render(<Toaster />);
    act(() => {
      toast({ kind: "error", title: "切换失败" });
    });

    act(() => {
      vi.advanceTimersByTime(9999);
    });
    expect(screen.getByRole("alert")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    act(() => {
      toast({ kind: "error", title: "写入失败" });
    });
    fireEvent.click(screen.getByRole("button", { name: "关闭通知" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("caps the stack at two and never drops an error", () => {
    render(<Toaster />);
    act(() => {
      toast({ kind: "info", title: "第一条" });
      toast({ kind: "info", title: "第二条" });
      toast({ kind: "info", title: "第三条" });
    });
    expect(screen.queryAllByRole("status")).toHaveLength(2);
    expect(screen.queryByText("第一条")).not.toBeInTheDocument();

    act(() => {
      toast({ kind: "error", title: "写入失败" });
    });
    const remaining = [...screen.queryAllByRole("status"), ...screen.queryAllByRole("alert")];
    expect(remaining).toHaveLength(2);
    expect(screen.getByRole("alert")).toHaveTextContent("写入失败");
    expect(screen.queryByText("第二条")).not.toBeInTheDocument();
  });

  it("rejects toasts without content", () => {
    render(<Toaster />);
    expect(() =>
      act(() => {
        toast({ kind: "info", title: "   " });
      }),
    ).toThrow(TypeError);
  });
});
