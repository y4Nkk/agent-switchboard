import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";
import react from "@vitejs/plugin-react";

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixturesPath = path.resolve(process.argv[2] ?? "");
assert.ok(fixturesPath.startsWith(path.join(repository, ".tmp") + path.sep));
const fixtures = JSON.parse(await fs.readFile(fixturesPath, "utf8"));
assert.equal(fixtures.passed, true);
const outDir = path.join(path.dirname(fixturesPath), "error-feedback-render");
await build({ root: repository, configFile: false, plugins: [react()], logLevel: "error",
  build: { ssr: path.join(repository, "scripts/command-error-lines.test.tsx"), outDir, emptyOutDir: false } });
const { runChecks } = await import(pathToFileURL(path.join(outDir, "command-error-lines.test.js")).href);
const result = runChecks(fixtures.errors);
await fs.writeFile(path.join(path.dirname(fixturesPath), "ui-evidence.json"), JSON.stringify(result, null, 2));
console.log(JSON.stringify(result));
