import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { UpdateButton } from "./UpdateButton";

describe("UpdateButton", () => {
  it("carries the discovered version in its accessible name", () => {
    render(<UpdateButton latestVersion="v0.2.0" onOpen={() => {}} />);

    expect(screen.getByRole("button", { name: "发现新版本 v0.2.0" })).toBeInTheDocument();
    expect(screen.getByText("更新")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "发现新版本 v0.2.0" }).querySelector(".asb-updatebtn-dot")).not.toBeNull();
  });

  it("delegates the click to the settings navigation", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    render(<UpdateButton latestVersion="v0.2.0" onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: "发现新版本 v0.2.0" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});
