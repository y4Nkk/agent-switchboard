import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FontPicker } from "./FontPicker";
import { listSystemFonts } from "../api/client";

vi.mock("../api/client", () => ({
  listSystemFonts: vi.fn(),
}));

const listSystemFontsMock = vi.mocked(listSystemFonts);

function optionRow(name: string) {
  return screen.getByRole("option", { name: new RegExp(name) });
}

describe("FontPicker", () => {
  beforeEach(() => {
    listSystemFontsMock.mockReset();
    listSystemFontsMock.mockResolvedValue(["Microsoft YaHei", "等线", "Segoe UI"]);
  });

  it("renders the current font in its own family on the trigger", async () => {
    render(<FontPicker value="Microsoft YaHei" busy={false} onChange={vi.fn()} />);

    const trigger = await screen.findByRole("button", { name: "选择界面字体" });
    const name = trigger.querySelector<HTMLElement>(".asb-font-name");
    expect(name?.textContent).toBe("Microsoft YaHei");
    expect(name?.style.fontFamily).toContain('"Microsoft YaHei"');
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
  });

  it("offers bundled plus installed fonts, each previewed in its own family", async () => {
    const user = userEvent.setup();
    render(<FontPicker value="Microsoft YaHei" busy={false} onChange={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "选择界面字体" }));

    expect(listSystemFontsMock).toHaveBeenCalledOnce();
    const options = screen.getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "Noto Sans SC中文 Aa 012",
      "Microsoft YaHei中文 Aa 012",
      "等线中文 Aa 012",
      "Segoe UI中文 Aa 012",
    ]);
    expect(
      optionRow("等线").querySelector<HTMLElement>(".asb-font-option-name")?.style.fontFamily,
    ).toContain('"等线"');
    expect(optionRow("Microsoft YaHei").getAttribute("aria-selected")).toBe("true");
    expect(optionRow("Segoe UI").getAttribute("aria-selected")).toBe("false");
  });

  it("filters options by the search query", async () => {
    const user = userEvent.setup();
    render(<FontPicker value="Noto Sans SC" busy={false} onChange={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "选择界面字体" }));
    await user.type(screen.getByLabelText("搜索字体"), "sego");

    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "Segoe UI中文 Aa 012",
    ]);
  });

  it("emits the chosen font, closes the menu, and refocuses the trigger", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<FontPicker value="Noto Sans SC" busy={false} onChange={onChange} />);

    const trigger = screen.getByRole("button", { name: "选择界面字体" });
    await user.click(trigger);
    await user.click(optionRow("等线"));

    expect(onChange).toHaveBeenCalledWith("等线");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(trigger).toHaveFocus();
  });

  it("closes on Escape and on an outside pointer press", async () => {
    const user = userEvent.setup();
    render(
      <>
        <FontPicker value="Noto Sans SC" busy={false} onChange={vi.fn()} />
        <button type="button">外部</button>
      </>,
    );

    await user.click(screen.getByRole("button", { name: "选择界面字体" }));
    expect(screen.getByRole("listbox")).toBeDefined();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();

    await user.click(screen.getByRole("button", { name: "选择界面字体" }));
    await user.click(screen.getByRole("button", { name: "外部" }));
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("moves focus with arrow keys inside the option list", async () => {
    const user = userEvent.setup();
    render(<FontPicker value="Noto Sans SC" busy={false} onChange={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "选择界面字体" }));
    await user.type(screen.getByLabelText("搜索字体"), "a{ArrowDown}");

    // Focus starts in the search field; the first ArrowDown reaches option 1.
    expect(optionRow("Noto Sans SC")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(optionRow("Microsoft YaHei")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(optionRow("Noto Sans SC")).toHaveFocus();
  });

  it("still offers the bundled default when enumeration fails", async () => {
    listSystemFontsMock.mockRejectedValue(new Error("unavailable"));
    render(<FontPicker value="Noto Sans SC" busy={false} onChange={vi.fn()} />);

    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "选择界面字体" }));

    await waitFor(() =>
      expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
        "Noto Sans SC中文 Aa 012",
      ]),
    );
    expect(screen.queryByText("没有找到相关字体")).toBeNull();
  });

  it("disables the trigger while busy", () => {
    render(<FontPicker value="Noto Sans SC" busy onChange={vi.fn()} />);

    expect(screen.getByRole("button", { name: "选择界面字体" })).toBeDisabled();
  });
});
