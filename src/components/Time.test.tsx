import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { render, screen } from "@testing-library/react";
import { Time } from "./Time";

const srcRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

function collectSourceFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === "test") continue;
      files.push(...collectSourceFiles(full));
    } else if (/\.(ts|tsx)$/.test(entry) && !/\.test\./.test(entry)) {
      files.push(full);
    }
  }
  return files;
}

describe("Time", () => {
  it("renders the local time without an offset suffix", () => {
    render(<Time iso="2026-08-26T08:00:00.000+00:00" />);
    const node = screen.getByText("2026年08月26日 16：00");
    expect(node).toBeInTheDocument();
    expect(node).toHaveAttribute("datetime", "2026-08-26T08:00:00.000+00:00");
  });

  it("is the only consumer of the time formatter outside lib/time.ts", () => {
    const offenders: string[] = [];
    for (const file of collectSourceFiles(srcRoot)) {
      const rel = relative(srcRoot, file).replace(/\\/g, "/");
      if (rel === "lib/time.ts" || rel === "components/Time.tsx") continue;
      if (readFileSync(file, "utf8").includes("timeLabel(")) {
        offenders.push(`${rel} formats time directly`);
      }
    }
    expect(offenders).toEqual([]);
  });
});
