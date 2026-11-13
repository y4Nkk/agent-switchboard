import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, realpath, rm } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { updateExamples } from "./fixtures/update-state.ts";

test("update surfaces render the same current contract with isolated output and no network", async () => {
  const repository = fileURLToPath(new URL("../", import.meta.url));
  const sandbox = path.join(repository, ".tmp");
  await mkdir(sandbox, { recursive: true });
  const outDir = await mkdtemp(path.join(sandbox, "update-render-"));
  const { stdout } = await promisify(execFile)(process.execPath, ["scripts/test-sidebar-update.mjs", outDir], {
    cwd: repository, windowsHide: true, timeout: 60_000,
  });
  const result = JSON.parse(stdout.trim());
  assert.equal(result.passed, true);
  assert.equal(result.nativeCommandsInvoked, false);
  assert.equal(result.nativeWindowAccessed, false);
  assert.equal(result.networkRequests, 0);
  assert.equal(result.cssCompiled, true);
  const examples = updateExamples().length;
  assert.equal(result.sidebarRenders, examples * 4);
  assert.equal(result.panelRenders, examples * 2);
  assert.equal(result.sectionRenders, examples * 2 + 6);
  const evidence = JSON.parse(await readFile(path.join(outDir, "render-evidence.json"), "utf8"));
  assert.equal(evidence.passed, true);
  assert.equal(evidence.sidebarRenders, result.sidebarRenders);
  const sandboxPath = await realpath(sandbox);
  const artifactPath = await realpath(outDir);
  const relative = path.relative(sandboxPath, artifactPath);
  assert.ok(relative && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
  await rm(artifactPath, { recursive: true });
});
