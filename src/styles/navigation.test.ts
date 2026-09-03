import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const baseCss = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "base.css"), "utf8");

function rule(selector: RegExp): string {
  return baseCss.match(selector)?.[1] ?? "";
}

describe("primary navigation state", () => {
  it("uses text hierarchy for the current page instead of a selected surface", () => {
    const idle = rule(/\.asb-nav button\s*\{([^}]*)\}/);
    const hover = rule(/\.asb-nav button:hover\s*\{([^}]*)\}/);
    const current = rule(/\.asb-nav button\[aria-current="page"\]\s*\{([^}]*)\}/);

    expect(idle).toContain("font-weight: 500");
    expect(hover).toContain("color: var(--asb-text)");
    expect(hover).not.toMatch(/(?:background|border)/);
    expect(current).toContain("color: var(--asb-text)");
    expect(current).toContain("font-weight: 700");
    expect(current).not.toMatch(/(?:background|border|text-decoration)/);
  });
});
