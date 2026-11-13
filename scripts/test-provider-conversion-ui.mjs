import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import http from "node:http";
import { build } from "vite";
import react from "@vitejs/plugin-react";

const repository = fileURLToPath(new URL("../", import.meta.url));
const output = path.resolve(process.env.ASB_PROVIDER_CONVERSION_UI_OUTPUT ?? path.join(repository, ".tmp", "provider-conversion-ui"));
const relative = path.relative(path.join(repository, ".tmp"), output);
assert.ok(relative && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
await fs.mkdir(output, { recursive: true });
const { chromium } = await import(process.env.ASB_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.ASB_PLAYWRIGHT_MODULE).href : "playwright-core");
const dist = path.join(output, "dist");
await build({ root: repository, configFile: false, mode: "test", plugins: [react()], logLevel: "error",
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: { lib: { entry: path.join(repository, "scripts/provider-conversion-ui.fixture.tsx"), formats: ["es"], fileName: "fixture" },
    outDir: dist, emptyOutDir: false, minify: false } });
const css = (await fs.readdir(dist)).find((name) => name.endsWith(".css"));
const server = http.createServer(async (request, response) => {
  const name = new URL(request.url, "http://localhost").pathname.slice(1);
  if (name === "favicon.ico") { response.writeHead(204).end(); return; }
  if (!name) { response.setHeader("Content-Type", "text/html"); response.end(`<html><link rel="stylesheet" href="/${css}"><div id="root"></div><script type="module" src="/fixture.js"></script></html>`); return; }
  if (!/^[a-zA-Z0-9._-]+$/.test(name)) { response.writeHead(404).end(); return; }
  try {
    response.setHeader("Content-Type", name.endsWith(".css") ? "text/css" : "text/javascript");
    response.end(await fs.readFile(path.join(dist, name)));
  } catch { response.writeHead(404).end(); }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const context = await chromium.launchPersistentContext(path.join(output, "browser-profile"), {
  headless: true, executablePath: process.env.ASB_BROWSER_EXECUTABLE,
  args: ["--no-proxy-server", "--disable-gpu", "--no-sandbox"], viewport: { width: 1280, height: 1000 }, timeout: 30000 });
const errors = [];
const blockedRequests = [];
await context.route("**/*", async (route) => {
  if (new URL(route.request().url()).origin === origin) await route.continue();
  else { blockedRequests.push(route.request().url()); await route.abort(); }
});
const page = context.pages()[0];
page.setDefaultTimeout(10000);
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
let assertions = 0;
const check = (value) => { assert.ok(value); assertions++; };
const alpha = () => page.getByRole("button", { name: "Convert Provider Alpha and add to Claude", exact: true });
const beta = () => page.getByRole("button", { name: "Convert Provider Beta and add to Codex", exact: true });
async function convert(button) {
  await button.locator("xpath=ancestor::li[1]").hover();
  await button.click();
}
async function checkBox(name) {
  const checkbox = page.getByRole("checkbox", { name, exact: true });
  if (!await checkbox.isChecked()) { await checkbox.focus(); await checkbox.press("Space"); }
}
async function checkLateSave(command, targetApp, shouldCreate) {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await convert(targetApp === "codex" ? beta() : alpha());
  if (targetApp === "codex") {
    await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
    await page.getByRole("spinbutton", { name: "vendor-beta context window", exact: true }).fill("32768");
    await page.getByRole("spinbutton", { name: "vendor-beta output limit", exact: true }).fill("2048");
    await checkBox("I have reviewed and confirmed the model capabilities");
  }
  await page.evaluate((name) => window.conversionTest.delaySave(name), command);
  await page.getByRole("button", { name: targetApp === "codex" ? "Add to Codex" : "Add to Claude", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().waitingSave);
  await page.evaluate(() => { window.conversionTest.newClaude(); window.conversionTest.releaseSave(); });
  await page.waitForFunction(() => !window.conversionTest.snapshot().busy);
  const result = await page.evaluate(() => window.conversionTest.snapshot());
  check(result.session?.app === "claude" && result.session.conversion === undefined);
  check(result.created.length === (shouldCreate ? 1 : 0));
  check(result.target === null);
  check(result.sourcesUnchanged);
  if (!shouldCreate) {
    const calls = await page.evaluate(() => window.conversionTest.calls);
    check(!calls.some(({ command }) => command === "commit_profile_save"));
    check(calls.some(({ command }) => command === "cancel_profile_save"));
  }
}
async function checkTouchControls() {
  const touch = await context.browser().newContext({ hasTouch: true, viewport: { width: 420, height: 900 } });
  await touch.route("**/*", async (route) => {
    if (new URL(route.request().url()).origin === origin) await route.continue();
    else { blockedRequests.push(route.request().url()); await route.abort(); }
  });
  try {
    const touchPage = await touch.newPage();
    touchPage.on("pageerror", (error) => errors.push(error.message));
    await touchPage.goto(origin, { waitUntil: "domcontentloaded" });
    const button = touchPage.getByRole("button", { name: "Convert Provider Alpha and add to Claude", exact: true });
    await button.waitFor();
    check(await touchPage.evaluate(() => matchMedia("(hover: none)").matches));
    check(await button.evaluate((node) => getComputedStyle(node.closest(".asb-row-action")).opacity === "1"));
    check(await touchPage.locator("[data-provider-conversion-id]").evaluateAll((buttons) => buttons.every((node) => {
      const button = node.getBoundingClientRect();
      const row = node.closest(".asb-row-item").getBoundingClientRect();
      return button.left >= row.left && button.right <= row.right;
    })));
    check(await touchPage.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
    await touchPage.screenshot({ path: path.join(output, "source-rows-touch-420.png"), fullPage: true, animations: "disabled" });
    await button.tap();
    await touchPage.getByRole("region", { name: "Provider conversion", exact: true }).waitFor();
    check((await touchPage.evaluate(() => window.conversionTest.snapshot())).session.app === "claude");
  } finally { await touch.close(); }
}
try {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await page.setViewportSize({ width: 640, height: 1000 });
  await alpha().locator("xpath=ancestor::li[1]").hover();
  check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await page.screenshot({ path: path.join(output, "source-rows-en-640.png"), fullPage: true, animations: "disabled" });
  await page.setViewportSize({ width: 1280, height: 1000 });
  await convert(alpha());
  await page.getByRole("region", { name: "Provider conversion", exact: true }).waitFor();
  check(await page.getByRole("checkbox", { name: "Copy usage query with manual refresh only" }).isChecked() === false);
  await checkBox("Copy usage query with manual refresh only");
  await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
  const mainModel = page.getByRole("textbox", { name: "Model", exact: true });
  await mainModel.waitFor();
  check(await mainModel.inputValue() === "vendor-fast");
  const claudeTargetIdentity = await page.evaluate(() => window.conversionTest.snapshot().session.profileId);
  await page.evaluate(() => window.conversionTest.failNextSave());
  await page.getByRole("button", { name: "Add to Claude", exact: true }).click();
  await page.getByRole("alert").filter({ hasText: "Isolated save failure" }).waitFor();
  check(await mainModel.inputValue() === "vendor-fast");
  const beforeDuplicate = await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_profile_save").length);
  await page.locator("form").evaluate((form) => { form.requestSubmit(); form.requestSubmit(); });
  await alpha().waitFor();
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_profile_save").length)) === beforeDuplicate + 1);
  let snapshot = await page.evaluate(() => window.conversionTest.snapshot());
  check(snapshot.created.length === 1 && snapshot.created[0].profile.id !== "codex-source");
  check(snapshot.created[0].profile.id === claudeTargetIdentity);
  check(snapshot.created[0].profile.usageQuery.refreshIntervalMinutes === 0);
  check(snapshot.sourcesUnchanged);
  await convert(beta());
  const codexTargetIdentity = await page.evaluate(() => window.conversionTest.snapshot().session.profileId);
  await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
  const contextLimit = page.getByRole("spinbutton", { name: "vendor-beta context window", exact: true });
  const outputLimit = page.getByRole("spinbutton", { name: "vendor-beta output limit", exact: true });
  await contextLimit.waitFor();
  check(await contextLimit.inputValue() === "" && await outputLimit.inputValue() === "");
  const beforeIncomplete = await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "create_codex_profile").length);
  await page.getByRole("button", { name: "Add to Codex", exact: true }).click();
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "create_codex_profile").length)) === beforeIncomplete);
  await contextLimit.fill("32768");
  await outputLimit.fill("2048");
  await page.getByRole("button", { name: "Add to Codex", exact: true }).click();
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "create_codex_profile").length)) === beforeIncomplete);
  await checkBox("I have reviewed and confirmed the model capabilities");
  check(await page.getByRole("button", { name: "Add to Codex", exact: true }).isEnabled());
  await outputLimit.fill("");
  await page.getByRole("button", { name: "Add to Codex", exact: true }).click();
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "create_codex_profile").length)) === beforeIncomplete);
  await outputLimit.fill("2048");
  await checkBox("I have reviewed and confirmed the model capabilities");
  await page.setViewportSize({ width: 640, height: 1000 });
  await page.evaluate(() => window.conversionTest.setLanguage("zh-CN"));
  check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await page.screenshot({ path: path.join(output, "reverse-zh-light-640.png"), fullPage: true, animations: "disabled" });
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await page.screenshot({ path: path.join(output, "reverse-zh-dark-640.png"), fullPage: true, animations: "disabled" });
  await page.getByRole("button", { name: "新增到 Codex", exact: true }).click();
  await page.getByRole("region", { name: "Saved sources" }).waitFor();
  snapshot = await page.evaluate(() => window.conversionTest.snapshot());
  check(snapshot.created.length === 2 && snapshot.created[1].profile.id !== "claude-source");
  check(snapshot.created[1].profile.id === codexTargetIdentity);
  check(snapshot.created[1].profile.catalog[0].contextWindow === 32768);
  check(snapshot.created[1].profile.usageQuery === null);
  await page.evaluate(() => window.conversionTest.setLanguage("en-US"));
  await convert(beta());
  await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
  await contextLimit.waitFor();
  check(await contextLimit.inputValue() === "");
  check(await page.getByRole("checkbox", { name: "I have reviewed and confirmed the model capabilities" }).isChecked() === false);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await beta().waitFor();
  check(await beta().evaluate((node) => node === document.activeElement));
  await convert(page.getByRole("button", { name: "Convert Official login and add to Codex", exact: true }));
  await page.getByRole("dialog", { name: "This configuration cannot be converted across clients" }).waitFor();
  check((await page.getByRole("dialog").innerText()).includes("Official login cannot be converted"));
  await page.keyboard.press("Escape");
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  await page.evaluate(() => window.conversionTest.delayNext());
  await convert(alpha());
  await page.waitForFunction(() => window.conversionTest.snapshot().waiting);
  await page.evaluate(() => { window.conversionTest.newClaude(); window.conversionTest.release(); });
  await page.waitForFunction(() => !window.conversionTest.snapshot().busy);
  check(await page.getByRole("region", { name: "Provider conversion", exact: true }).count() === 0);
  check((await page.evaluate(() => window.conversionTest.snapshot())).session.conversion === undefined);
  await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
  check(await page.getByRole("textbox", { name: "Model", exact: true }).inputValue() === "");
  const commands = await page.evaluate(() => window.conversionTest.calls);
  check(!commands.some(({ command }) => /execute_switch|prepare_codex_profile_save|query.*usage|fetch_provider_models|test_provider/.test(command)));
  check((await page.evaluate(() => window.conversionTest.snapshot())).sourcesUnchanged);
  await checkLateSave("prepare_profile_save", "claude", false);
  await checkLateSave("commit_profile_save", "claude", true);
  await checkLateSave("create_codex_profile", "codex", true);
  await checkTouchControls();
  check(blockedRequests.length === 0);
  check(errors.length === 0);
  await fs.writeFile(path.join(output, "evidence.json"), `${JSON.stringify({ passed: true, assertions,
    realProviderRequests: 0, nativeConfigurationWrites: 0, errors, blockedRequests }, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ passed: true, assertions, output })}\n`);
} catch (error) {
  await fs.writeFile(path.join(output, "failure.json"), `${JSON.stringify({ error: error.message, errors,
    snapshot: await page.evaluate(() => window.conversionTest?.snapshot()),
    calls: await page.evaluate(() => window.conversionTest?.calls),
    text: await page.locator("body").innerText() }, null, 2)}\n`);
  await page.screenshot({ path: path.join(output, "failure.png"), fullPage: true });
  throw error;
} finally {
  await context.close();
  await new Promise((resolve) => server.close(resolve));
}
