import { spawn } from "node:child_process";
import { createWriteStream, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { prepareRealFixtures } from "./copy-provider-conversion-fixtures.mjs";

const repository = fileURLToPath(new URL("../", import.meta.url));
const outputRoot = path.join(repository, ".tmp");
mkdirSync(outputRoot, { recursive: true });
const output = mkdtempSync(path.join(outputRoot, "provider-conversion-"));
const sandboxPaths = Object.fromEntries(["home", "appdata", "localappdata", "codex", "claude", "temp"]
  .map((name) => [name, path.join(output, "environment", name)]));
for (const directory of Object.values(sandboxPaths)) mkdirSync(directory, { recursive: true });
const fixtures = prepareRealFixtures(output);
const isolatedEnvironment = {
  ...process.env,
  CARGO_HOME: process.env.CARGO_HOME || path.join(homedir(), ".cargo"),
  RUSTUP_HOME: process.env.RUSTUP_HOME || path.join(homedir(), ".rustup"),
  HOME: sandboxPaths.home,
  USERPROFILE: sandboxPaths.home,
  APPDATA: sandboxPaths.appdata,
  LOCALAPPDATA: sandboxPaths.localappdata,
  XDG_CONFIG_HOME: path.join(sandboxPaths.home, ".config"),
  XDG_CACHE_HOME: path.join(sandboxPaths.home, ".cache"),
  XDG_DATA_HOME: path.join(sandboxPaths.home, ".local", "share"),
  CODEX_HOME: sandboxPaths.codex,
  CLAUDE_CONFIG_DIR: sandboxPaths.claude,
  TMPDIR: sandboxPaths.temp,
  TEMP: sandboxPaths.temp,
  TMP: sandboxPaths.temp,
  ...fixtures.env,
  ASB_PROVIDER_CONVERSION_UI_OUTPUT: path.join(output, "ui"),
};
for (const name of Object.keys(isolatedEnvironment)) {
  if (name === "ASB_RUN_LIVE_CODEX_TESTS" || name.startsWith("ASB_CODEX_LIVE_")
    || /(?:API_KEY|ACCESS_TOKEN|AUTH_TOKEN)$/.test(name)) delete isolatedEnvironment[name];
}
const native = ["test", "-p", "agent-switchboard", "--lib", "--offline", "--locked"];
const steps = [
  ["core", "cargo", ["test", "-p", "asb-core", "--offline", "--locked", "provider_conversion", "--", "--test-threads=1"]],
  ["native", "cargo", [...native, "provider_conversion::tests::", "--", "--include-ignored", "--test-threads=1"]],
  ["claude-models", "cargo", [...native, "gateway::transform::claude_model", "--", "--test-threads=1"]],
  ["frontend", process.execPath, ["--test", "tests/provider-conversion.test.ts", "tests/model-routing.test.ts", "tests/i18n.test.ts"]],
  ["ui", process.execPath, ["scripts/test-provider-conversion-ui.mjs"]],
];
const requested = new Set(process.argv.slice(2));
const report = { passed: false, fixtures: fixtures.summary, steps: [], originals: null, copiesRemoved: false };

async function runStep(name, command, args) {
  const log = createWriteStream(path.join(output, `${name}.log`));
  const started = Date.now();
  process.stdout.write(`[provider-conversion] ${name}\n`);
  const code = await new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: repository, env: isolatedEnvironment, windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"] });
    child.on("error", reject);
    child.stdout.on("data", (data) => { process.stdout.write(data); log.write(data); });
    child.stderr.on("data", (data) => { process.stderr.write(data); log.write(data); });
    child.on("close", resolve);
  }).finally(() => log.end());
  report.steps.push({ name, code, durationMs: Date.now() - started });
  if (code !== 0) throw new Error(`${name} exited with ${code}`);
}

function removeIsolatedEnvironment() {
  const canonicalOutput = realpathSync(output);
  const target = realpathSync(path.join(output, "environment"));
  const suffix = path.relative(canonicalOutput, target);
  if (suffix !== "environment" || path.relative(realpathSync(outputRoot), canonicalOutput).startsWith("..")) {
    throw new Error("Refusing to remove an unverified test environment");
  }
  rmSync(target, { recursive: true, maxRetries: 10, retryDelay: 100 });
}

try {
  for (const name of requested) {
    if (!steps.some(([step]) => step === name)) throw new Error(`Unknown verification step: ${name}`);
  }
  for (const [name, command, args] of steps) {
    if (!requested.size || requested.has(name)) await runStep(name, command, args);
  }
  report.passed = true;
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
} finally {
  try { report.originals = fixtures.verifyOriginals(); }
  catch (error) { report.passed = false; process.exitCode = 1; process.stderr.write(`${error.message}\n`); }
  try { fixtures.cleanup(); removeIsolatedEnvironment(); report.copiesRemoved = true; }
  catch (error) { report.passed = false; process.exitCode = 1; process.stderr.write(`${error.message}\n`); }
  writeFileSync(path.join(output, "summary.json"), `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ passed: report.passed, evidence: path.join(output, "summary.json") })}\n`);
}
