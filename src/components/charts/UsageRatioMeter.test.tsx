import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { UsageRatioMeter } from "./UsageRatioMeter";

describe("UsageRatioMeter", () => {
  it("keeps the percentage text beside the compact progress rail", () => {
    render(<UsageRatioMeter percent={42.5} ariaLabel="主套餐已用比例" />);
    const meter = screen.getByRole("progressbar", { name: "主套餐已用比例" });
    expect(meter).toHaveAttribute("aria-valuenow", "42.5");
    expect(meter).toHaveTextContent("42.5 %");
  });

  it("clamps only the rail while preserving an out-of-range source ratio", () => {
    const { rerender } = render(<UsageRatioMeter percent={140} ariaLabel="窗口已用比例" />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "100");
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuetext", "140 %");
    expect(screen.getByText("140 %")).toBeInTheDocument();

    rerender(<UsageRatioMeter percent={null} ariaLabel="窗口已用比例" />);
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.getByLabelText("窗口已用比例")).toHaveTextContent("—");
  });
});
