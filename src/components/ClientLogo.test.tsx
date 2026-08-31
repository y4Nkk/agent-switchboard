import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { ClientLogo } from "./ClientLogo";

describe("ClientLogo", () => {
  it("renders one distinct brand mark per client with the passed class", () => {
    // Vite inlines assets as data URIs in tests, so only distinctness is stable.
    const codex = render(<ClientLogo app="codex" className="asb-status-logo" />);
    const codexImg = codex.container.querySelector("img.asb-status-logo");
    expect(codexImg).not.toBeNull();
    expect(codexImg?.getAttribute("src")?.length ?? 0).toBeGreaterThan(0);

    const claude = render(<ClientLogo app="claude" className="asb-route-logo" />);
    const claudeImg = claude.container.querySelector("img.asb-route-logo");
    expect(claudeImg).not.toBeNull();
    expect(claudeImg?.getAttribute("src")).not.toBe(codexImg?.getAttribute("src"));
  });

  it("stays decorative: empty alt, so the adjacent client name carries meaning", () => {
    const { container } = render(<ClientLogo app="codex" className="x" />);
    expect(container.querySelector("img")).toHaveAttribute("alt", "");
  });
});
