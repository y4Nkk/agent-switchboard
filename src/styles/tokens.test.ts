import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const srcRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const tokenOwner = "styles/tokens.css";

function collectSourceFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === "test") continue;
      files.push(...collectSourceFiles(full));
    } else if (/\.(ts|tsx|css)$/.test(entry)) {
      files.push(full);
    }
  }
  return files;
}

describe("token ownership", () => {
  it("defines visual constants only in the token stylesheet", () => {
    const offenders: string[] = [];
    for (const file of collectSourceFiles(srcRoot)) {
      const rel = relative(srcRoot, file).replace(/\\/g, "/");
      if (rel === tokenOwner) continue;
      const text = readFileSync(file, "utf8");
      const hex = text.match(/#[0-9a-fA-F]{3,8}\b/g);
      if (hex) offenders.push(`${rel}: raw color ${hex.join(", ")}`);
    }
    expect(offenders).toEqual([]);
  });

  it("uses one opaque surface path without backdrop sampling or exit states", () => {
    const tokens = readFileSync(join(srcRoot, tokenOwner), "utf8");
    const base = readFileSync(join(srcRoot, "styles/base.css"), "utf8");

    expect(tokens).toContain("--asb-surface-rail:");
    expect(tokens).toContain("--asb-surface-menu:");
    expect(tokens).toContain("--asb-surface-sheet:");
    expect(tokens).not.toMatch(/--asb-(blur|glass|saturation)-/);
    expect(base).not.toMatch(/(?:-webkit-)?backdrop-filter|filter:\s*blur\(/);
    expect(base).not.toMatch(
      /button:not\(:disabled\):active|asb-(route-card-in|select-in|tooltip-in|sheet-in|toast-in|toast-out)|is-leaving/,
    );
  });

  it("keeps module perimeters neutral instead of using colored decorative strips", () => {
    const tokens = readFileSync(join(srcRoot, tokenOwner), "utf8");
    const base = readFileSync(join(srcRoot, "styles/base.css"), "utf8");

    expect(tokens).not.toMatch(/--asb-accent-(?:ring|border)-/);
    expect(base).not.toMatch(/conic-gradient\(/);
    expect(base).not.toMatch(
      /border-left:\s*\d+px\s+solid\s+var\(--asb-(?:action|warning|danger)\)/,
    );
  });

  it("supports explicit theme and motion overrides without a second stylesheet owner", () => {
    const tokens = readFileSync(join(srcRoot, tokenOwner), "utf8");
    expect(tokens).toContain(':root:not([data-theme="light"])');
    expect(tokens).toContain(':root[data-theme="dark"]');
    expect(tokens).toContain(':root[data-motion="reduce"]');
    expect(tokens).toContain('--asb-motion-fast: 0ms');
  });
});
