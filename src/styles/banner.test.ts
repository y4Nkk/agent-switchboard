import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/* The banner layer is click-through by design (DESIGN.md §7), so interactive
   controls inside a banner must opt back into pointer events in CSS. jsdom
   never loads stylesheets, so this contract can only be locked by scanning
   the stylesheet text — the same approach boundary.test.ts uses for code. */
const baseCss = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "base.css"),
  "utf8",
);

describe("banner pointer-events contract", () => {
  it("keeps the banner stack click-through while banner buttons opt back in", () => {
    const stackBlock = baseCss.match(/\.asb-banner-stack \{[^}]+\}/)?.[0] ?? "";
    expect(stackBlock).toContain("pointer-events: none");

    expect(baseCss).toMatch(/\.asb-banner > button \{[^}]*pointer-events: auto;/);
  });
});
