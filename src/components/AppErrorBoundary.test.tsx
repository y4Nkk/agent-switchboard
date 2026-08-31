import { type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { AppErrorBoundary } from "./AppErrorBoundary";

function ThrowingSurface(): ReactNode {
  throw new Error("render failed");
}

describe("AppErrorBoundary", () => {
  it("replaces a failed workspace with a recovery surface", () => {
    using _consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <AppErrorBoundary>
        <ThrowingSurface />
      </AppErrorBoundary>,
    );

    expect(screen.getByRole("alert", { name: "界面恢复" })).toBeInTheDocument();
    expect(screen.getByText("界面未能加载")).toBeInTheDocument();
    expect(screen.getByText(/系统托盘重新打开应用/)).toBeInTheDocument();
  });
});
