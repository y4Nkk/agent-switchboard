import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const baseCss = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "base.css"), "utf8");

function expectRule(selector: string, declaration: string) {
  expect(baseCss).toMatch(new RegExp(`${selector}\\s*\\{[^}]*${declaration}`));
}

describe("visually hidden native control positioning", () => {
  it("anchors each native input to its drawn control", () => {
    expectRule("\\.asb-seg-opt", "position:\\s*relative\\s*;");
    expectRule("\\.asb-seg-opt input", "position:\\s*absolute\\s*;");
    expectRule("\\.asb-checkbox", "position:\\s*relative\\s*;");
    expectRule("\\.asb-checkbox-input", "position:\\s*absolute\\s*;");
    expectRule("\\.asb-switch", "position:\\s*relative\\s*;");
    expectRule("\\.asb-switch-input", "position:\\s*absolute\\s*;");
  });
});
