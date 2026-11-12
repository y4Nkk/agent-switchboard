import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";
import react from "@vitejs/plugin-react";

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const evidencePath = path.resolve(process.argv[2] ?? "");
assert.ok(evidencePath.startsWith(path.join(repository, ".tmp", "application-preparation-sandbox") + path.sep));
const backend = JSON.parse(await fs.readFile(evidencePath, "utf8"));
assert.equal(backend.passed, true);
const sandbox = path.resolve(backend.sandbox);
const outDir = path.join(sandbox, "ui-render");
assert.ok(outDir.startsWith(sandbox + path.sep));
await build({ root: repository, configFile: false, plugins: [react()], logLevel: "error",
  build: { ssr: path.join(repository, "scripts/application-preparation-ui.test.tsx"), outDir, emptyOutDir: false } });
const { runChecks } = await import(pathToFileURL(path.join(outDir, "application-preparation-ui.test.js")).href);
const report = JSON.parse(await fs.readFile(path.join(sandbox, "configuration-enabled", "result.json"), "utf8"));
const result = runChecks(report);
const styles = await fs.readFile(path.join(repository, "src/styles/base/application-preparation.css"), "utf8");
assert.match(styles, /prefers-reduced-motion:\s*reduce/);
assert.match(styles, /data-motion="reduce"/);
result.assertions += 2;
await fs.writeFile(path.join(sandbox, "ui-evidence.json"), JSON.stringify(result, null, 2));
console.log(JSON.stringify(result));
