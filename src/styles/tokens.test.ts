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
      const rawBlur = text.match(/(?:backdrop-filter|-webkit-backdrop-filter)[^;]*blur\(\s*\d/g);
      if (rawBlur) offenders.push(`${rel}: raw blur length`);
    }
    expect(offenders).toEqual([]);
  });

  it("supports explicit theme and motion overrides without a second stylesheet owner", () => {
    const tokens = readFileSync(join(srcRoot, tokenOwner), "utf8");
    expect(tokens).toContain(':root:not([data-theme="light"])');
    expect(tokens).toContain(':root[data-theme="dark"]');
    expect(tokens).toContain(':root[data-motion="reduce"]');
    expect(tokens).toContain('--asb-motion-fast: 0ms');
  });
});
