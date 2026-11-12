import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { pathToFileURL } from "node:url";
import { build } from "vite";
import http from "node:http";
import react from "@vitejs/plugin-react";

const root = process.cwd();
const output = path.join(root, ".tmp", "model-routing-ui");
await fs.mkdir(output, { recursive: true });
const playwright = process.env.ASB_PLAYWRIGHT_MODULE;
const { chromium } = await import(playwright ? pathToFileURL(playwright).href : "playwright-core");
const dist = path.join(output, "dist");
await build({ root, configFile: false, mode: "test", plugins: [react()], logLevel: "error",
  resolve: { alias: { "@": path.join(root, "src") } },
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: { lib: { entry: path.join(root, "scripts/model-routing-ui.fixture.tsx"), formats: ["es"], fileName: "fixture" },
    outDir: dist, emptyOutDir: false, minify: false },
});
const css = (await fs.readdir(dist)).find((file) => file.endsWith(".css"));
const html = `<html><link rel="stylesheet" href="/${css}"><div id="root"></div><script type="module" src="/fixture.js"></script></html>`;
const server = http.createServer(async (request, response) => {
  const name = new URL(request.url, "http://localhost").pathname.slice(1);
  if (name === "favicon.ico") { response.writeHead(204).end(); return; }
  if (!name || name === "index.html") { response.setHeader("Content-Type", "text/html"); response.end(html); return; }
  if (!/^[a-zA-Z0-9._-]+$/.test(name)) { response.writeHead(404).end(); return; }
  try {
    response.setHeader("Content-Type", name.endsWith(".css") ? "text/css" : "text/javascript");
    response.end(await fs.readFile(path.join(dist, name)));
  } catch { response.writeHead(404).end(); }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}/`;
const browserProfile = await fs.mkdtemp(path.join(os.tmpdir(), "asb-routing-ui-"));
const context = await chromium.launchPersistentContext(browserProfile, {
  headless: true, args: ["--no-proxy-server", "--disable-gpu", "--no-sandbox"], timeout: 30000,
  viewport: { width: 1440, height: 1050 },
  executablePath: process.env.ASB_BROWSER_EXECUTABLE,
});
const errors = [];
const page = context.pages()[0] ?? await context.newPage();
page.setDefaultTimeout(15000);
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
let assertions = 0;
const check = (condition) => { assert.ok(condition); assertions++; };
async function chooseProvider(label, name) {
  await page.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name, exact: true }).click();
}
async function pickModel(label, name) {
  await page.getByRole("button", { name: label, exact: true }).click();
  await page.getByRole("option", { name, exact: true }).click();
}
try {
  await page.goto(origin, { waitUntil: "domcontentloaded", timeout: 15000 });
  await page.getByRole("combobox", { name: "Profile being edited" }).waitFor();
  check(await page.getByRole("combobox", { name: "Plan provider", exact: true }).count() === 0);
  await chooseProvider("Profile being edited", "Codex B");
  await pickModel("Model", "other-model");
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("dialog", { name: "Confirm model routing changes" }).waitFor();
  check((await page.getByRole("dialog").innerText()).includes("Primary provider stays unchanged"));
  check((await page.getByRole("dialog").innerText()).includes("Codex A"));
  check((await page.getByRole("dialog").innerText()).includes("This only saves the profile"));
  await page.getByRole("button", { name: "Save plan", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  const saved = await page.evaluate(() => window.routeTest.snapshot());
  check(saved.activeIds.codex === "Codex A");
  let calls = await page.evaluate(() => window.routeTest.calls);
  check(calls.find((call) => call.command === "prepare_codex_profile_save").args.intent === undefined);
  check(calls.find((call) => call.command === "commit_codex_profile_save").args.confirmWrite === false);
  await page.getByRole("button", { name: "Edit active plan", exact: true }).click();
  await pickModel("Model", "other-model");
  await page.evaluate(() => window.routeTest.setKind("saveAndApply"));
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("dialog").waitFor();
  check((await page.getByRole("dialog").innerText()).includes("local gateway"));
  await page.evaluate(() => { window.routeTest.failCancels(1); window.routeTest.switchActive("codex", "Codex B"); });
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  check((await page.getByRole("textbox", { name: "Model", exact: true }).inputValue()) === "other-model");
  check((await page.getByRole("combobox", { name: "Profile being edited" }).innerText()).includes("Codex A"));
  await page.waitForFunction(() => window.routeTest.calls.some((call) => call.command === "cancel_profile_save"));
  await page.getByRole("status").filter({ hasText: "remains queued for release" }).waitFor();
  check((await page.evaluate(() => window.routeTest.snapshot())).pendingCount === 1);
  const staleId = (await page.evaluate(() => window.routeTest.calls)).findLast((call) => call.command === "prepare_codex_profile_save").args;
  const preparationsBeforeRetry = (await page.evaluate(() => window.routeTest.calls)).filter((call) => call.command.startsWith("prepare_")).length;
  await page.evaluate(() => window.routeTest.failCancels(1));
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.waitForFunction(() => window.routeTest.calls.filter((call) => call.command === "cancel_profile_save").length === 2
    && [...document.querySelectorAll("button")].some((button) => button.textContent === "Preview save" && !button.disabled));
  check((await page.evaluate(() => window.routeTest.calls)).filter((call) => call.command.startsWith("prepare_")).length === preparationsBeforeRetry);
  check(await page.getByRole("dialog").count() === 0);
  check((await page.evaluate(() => window.routeTest.snapshot())).pendingCount === 1);
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("dialog").waitFor();
  calls = await page.evaluate(() => window.routeTest.calls);
  const cancellationIndices = calls.map((call, index) => call.command === "cancel_profile_save" ? index : -1).filter((index) => index >= 0);
  const retryIndex = cancellationIndices.at(-1);
  check(cancellationIndices.length === 3);
  check(calls[retryIndex + 1].command === "prepare_codex_profile_save");
  check(calls[retryIndex].args.preparationId === calls[cancellationIndices[0]].args.preparationId);
  check(calls.filter((call) => call.command === "commit_codex_profile_save").length === 1);
  check(staleId.profileId === "Codex A");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  check((await page.evaluate(() => window.routeTest.snapshot())).pendingCount === 0);
  await page.evaluate(() => window.routeTest.delayNext());
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.waitForFunction(() => window.routeTest.snapshot().waitingPreparation);
  await page.evaluate(() => { window.routeTest.switchActive("codex", "Codex A"); });
  await page.getByRole("status").filter({ hasText: "Currently active provider: Codex A" }).waitFor();
  await page.evaluate(() => window.routeTest.resolveDelayed());
  await page.waitForFunction(() => window.routeTest.snapshot().pendingCount === 0);
  check(await page.getByRole("dialog").count() === 0);
  check((await page.getByRole("textbox", { name: "Model", exact: true }).inputValue()) === "other-model");
  await page.goto(`${origin}?client=claude`);
  await chooseProvider("Profile being edited", "Claude B");
  await page.getByRole("textbox", { name: "Model", exact: true }).fill("other-model");
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("button", { name: "Save plan", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  check((await page.evaluate(() => window.routeTest.snapshot())).activeIds.claude === "Claude A");
  await page.getByRole("textbox", { name: "Model", exact: true }).fill("shared-model");
  await page.evaluate(() => window.routeTest.setKind("saveAndApply"));
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("dialog").waitFor();
  check((await page.getByRole("dialog").innerText()).includes("Claude A"));
  await page.getByRole("button", { name: "Save and update live routing", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  check((await page.evaluate(() => window.routeTest.snapshot())).activeIds.claude === "Claude A");
  await page.goto(`${origin}?client=claude`);
  await page.getByRole("button", { name: "Configure provider and model for Default subagent", exact: true }).click();
  await page.getByRole("radio", { name: "Choose provider and model", exact: true }).locator("..").click();
  await chooseProvider("Target provider", "Claude B");
  await pickModel("Choose a configured model", "shared-model");
  await page.evaluate(() => window.routeTest.setKind("saveAndApply"));
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("dialog").waitFor();
  check((await page.getByRole("dialog").innerText()).includes("Claude B / shared-model"));
  await page.screenshot({ path: path.join(output, "claude-dependent-preview.png"), fullPage: true });
  check((await page.getByRole("dialog").innerText()).includes("Isolated warning remains visible."));
  check(!(await page.getByRole("dialog").innerText()).includes("Current user-level configuration model"));
  await page.setViewportSize({ width: 640, height: 1050 });
  await page.evaluate(() => { window.routeTest.setLanguage("zh-CN"); document.documentElement.dataset.theme = "light"; });
  await page.getByRole("dialog", { name: "确认模型路由变更", exact: true }).waitFor();
  check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  check(await page.getByRole("dialog").evaluate((node) => node.scrollWidth <= node.clientWidth));
  await page.screenshot({ path: path.join(output, "claude-preview-zh-640.png"), fullPage: true });
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await page.screenshot({ path: path.join(output, "claude-preview-zh-dark-640.png"), fullPage: true });
  await page.evaluate(() => { window.routeTest.setLanguage("en-US"); document.documentElement.dataset.theme = "light"; });
  await page.setViewportSize({ width: 1440, height: 1050 });
  await page.getByRole("button", { name: "Save and update live routing", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  calls = await page.evaluate(() => window.routeTest.calls);
  const commit = calls.findLast((call) => call.command === "commit_profile_save");
  const preparation = calls.findLast((call) => call.command === "prepare_profile_save");
  check(commit.args.confirmWrite === true);
  check(preparation.args.draft.modelOptions.routes.subagent.profileId === "Claude B");
  check((await page.evaluate(() => window.routeTest.snapshot())).activeIds.claude === "Claude A");
  await page.evaluate(() => window.routeTest.removeClaudeTarget());
  await page.getByRole("status").filter({ hasText: "The referenced target provider no longer exists" }).waitFor();
  check(await page.getByRole("button", { name: "Preview save", exact: true }).isDisabled());
  await page.screenshot({ path: path.join(output, "claude-invalid-target.png"), fullPage: true });
  await page.goto(`${origin}?client=claude`);
  await page.getByRole("button", { name: "Configure provider and model for Sonnet tier", exact: true }).click();
  check((await page.getByRole("textbox", { name: "Model", exact: true }).inputValue()) === "unique-only-tier");
  await page.getByRole("radio", { name: "Choose provider and model", exact: true }).locator("..").click();
  await chooseProvider("Target provider", "Claude A");
  await pickModel("Choose a configured model", "unique-only-tier");
  await page.evaluate(() => window.routeTest.setKind("saveAndApply"));
  await page.getByRole("button", { name: "Preview save", exact: true }).click();
  await page.getByRole("button", { name: "Save and update live routing", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  calls = await page.evaluate(() => window.routeTest.calls);
  const selfRoute = calls.findLast((call) => call.command === "prepare_profile_save").args.draft.modelOptions.routes.sonnet;
  check(selfRoute.mode === "provider" && selfRoute.profileId === "Claude A" && selfRoute.model === "unique-only-tier");
  check((await page.evaluate(() => window.routeTest.snapshot())).activeIds.claude === "Claude A");
  assert.deepEqual(errors, []); assertions++;
  const result = { assertions, passed: true, errors, evidence: output,
    coverage: ["inactive Codex save retains A", "save-only confirm flag", "no activate argument", "main node has no provider selector",
      "live preview human summary", "active change cancels token and retains draft", "Claude default subagent cross-provider",
      "same model name retains selected provider", "dependent live save retains owner", "missing target blocks save", "failed lifecycle cancellation retained and retried before new prepare", "failed retry blocks new preparation", "late preview after context change discarded", "tier-only saved model remains selectable when switching to fixed self target"] };
  await fs.writeFile(path.join(output, "evidence.json"), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} catch (error) { console.error("fixture failure", error); throw error; }
finally { await context.close(); await new Promise((resolve) => server.close(resolve));
  if (path.dirname(browserProfile) === os.tmpdir()) await fs.rm(browserProfile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 }); }
