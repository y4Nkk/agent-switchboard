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
const snapshot = () => page.evaluate(() => window.conversionTest.snapshot());
const toolbar = () => page.getByRole("button", { name: "Convert providers", exact: true });
async function checkBox(name, targetPage = page) {
  const checkbox = targetPage.getByRole("checkbox", { name, exact: true });
  if (!await checkbox.isChecked()) { await checkbox.focus(); await checkbox.press("Space"); }
}
async function openPicker(app, targetPage = page) {
  const radio = targetPage.getByRole("radio", { name: app === "codex" ? "Codex" : "Claude", exact: true });
  if (!await radio.isChecked()) { await radio.focus(); await radio.press("Space"); }
  await targetPage.getByRole("button", { name: "Convert providers", exact: true }).click();
  await targetPage.getByRole("dialog", { name: app === "codex"
    ? "Select Codex providers to add to Claude" : "Select Claude providers to add to Codex", exact: true }).waitFor();
}
async function convert(app, names, targetPage = page) {
  await openPicker(app, targetPage);
  for (const name of names) await checkBox(name, targetPage);
  await targetPage.getByRole("button", { name: `Convert one by one (${names.length})`, exact: true }).click();
  await targetPage.waitForFunction(() => {
    const batch = window.conversionTest.snapshot().batch;
    return batch && !["preparing", "ready"].includes(batch.phase);
  });
}
async function completeCodexLimits() {
  await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
  await page.getByRole("spinbutton", { name: "vendor-beta context window", exact: true }).fill("32768");
  await page.getByRole("spinbutton", { name: "vendor-beta output limit", exact: true }).fill("2048");
  await page.getByRole("button", { name: "I have reviewed the model configuration", exact: true }).click();
}
async function closeResult() {
  await page.getByRole("dialog", { name: "Selected providers processed", exact: true }).waitFor();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  check(await toolbar().evaluate((node) => node === document.activeElement));
}
async function checkLateSave(command, app, shouldCreate) {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await convert(app, [app === "codex" ? "Provider Alpha" : "Provider Beta"]);
  if (app === "claude") await completeCodexLimits();
  await page.evaluate((name) => window.conversionTest.delaySave(name), command);
  await page.getByRole("button", { name: app === "codex" ? "Add to Claude" : "Add to Codex", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().waitingSave);
  await page.evaluate(() => { window.conversionTest.newClaude(); window.conversionTest.releaseSave(); });
  await page.waitForFunction(() => !window.conversionTest.snapshot().busy);
  const result = await snapshot();
  check(result.session?.app === "claude" && result.session.conversion === undefined);
  check(result.batch === null && result.created.length === Number(shouldCreate));
  check(result.target === null && result.sourcesUnchanged);
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
    const button = touchPage.getByRole("button", { name: "Convert providers", exact: true });
    check(await button.evaluate((node) => getComputedStyle(node).opacity === "1" && !node.closest(".asb-row-item")));
    check(await touchPage.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
    await convert("codex", ["Provider Alpha"], touchPage);
    check((await touchPage.evaluate(() => window.conversionTest.snapshot())).session.app === "claude");
    await touchPage.screenshot({ path: path.join(output, "conversion-touch-420.png"), fullPage: true, animations: "disabled" });
  } finally { await touch.close(); }
}
async function checkSaveWhileAway() {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await convert("codex", ["Provider Alpha", "Provider Gamma"]);
  await page.evaluate(() => window.conversionTest.delaySave("commit_profile_save"));
  await page.getByRole("button", { name: "Add to Claude", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().waitingSave);
  await page.evaluate(() => { window.conversionTest.setActive(false); window.conversionTest.releaseSave(); });
  await page.waitForFunction(() => !window.conversionTest.snapshot().busy);
  const result = await snapshot();
  check(result.created.length === 1 && result.batch.saved === 1 && result.batch.phase === "ready");
  check(result.session === null);
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_provider_conversion").length)) === 1);
  await page.evaluate(() => window.conversionTest.setActive(true));
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.index === 1 && window.conversionTest.snapshot().batch?.phase === "editing");
  check((await snapshot()).session.conversion.source.id === "codex-second");
  await page.getByRole("button", { name: "End conversion", exact: true }).click();
}
async function checkSaveCannotBeSkipped(app) {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await convert(app, app === "codex" ? ["Provider Alpha", "Provider Gamma"] : ["Provider Beta", "Official login"]);
  if (app === "claude") await completeCodexLimits();
  await page.evaluate((command) => {
    window.conversionTest.delaySave(command);
    document.querySelector("form").requestSubmit();
    window.conversionTest.skip();
    window.conversionTest.end();
  }, app === "codex" ? "commit_profile_save" : "create_codex_profile");
  await page.waitForFunction(() => window.conversionTest.snapshot().waitingSave);
  let result = await snapshot();
  check(result.batch.phase === "saving" && result.batch.index === 0 && result.batch.skipped === 0);
  check(await page.getByRole("button", { name: "Skip this provider", exact: true }).isDisabled());
  check(await page.getByRole("button", { name: "End conversion", exact: true }).isDisabled());
  await page.evaluate(() => window.conversionTest.releaseSave());
  await page.waitForFunction(() => {
    const batch = window.conversionTest.snapshot().batch;
    return batch?.index === 1 && ["editing", "blocked"].includes(batch.phase);
  });
  result = await snapshot();
  check(result.batch.saved === 1 && result.batch.skipped === 0 && result.created.length === 1);
  await page.getByRole("button", { name: "End conversion", exact: true }).click();
}
async function checkInvalidResult() {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await page.evaluate(() => window.conversionTest.invalidateNextResult());
  await convert("codex", ["Provider Alpha"]);
  await page.getByRole("alert").filter({ hasText: "missing a valid draft" }).waitFor();
  check((await snapshot()).session === null && (await snapshot()).created.length === 0);
  await page.getByRole("button", { name: "End conversion", exact: true }).click();
}
async function checkPickerPresentation() {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await page.setViewportSize({ width: 640, height: 1000 });
  check(await toolbar().evaluate((node) => !node.closest(".asb-row-item") && getComputedStyle(node).opacity === "1"));
  check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await page.screenshot({ path: path.join(output, "toolbar-en-640.png"), fullPage: true, animations: "disabled" });
  await openPicker("codex");
  await page.screenshot({ path: path.join(output, "picker-en-640.png"), fullPage: true, animations: "disabled" });
  await page.evaluate(() => window.conversionTest.setLanguage("zh-CN"));
  await page.getByRole("dialog", { name: "选择 Codex 供应商，新增到 Claude", exact: true }).waitFor();
  await page.screenshot({ path: path.join(output, "picker-zh-light-640.png"), fullPage: true, animations: "disabled" });
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await page.screenshot({ path: path.join(output, "picker-zh-dark-640.png"), fullPage: true, animations: "disabled" });
  check(await page.getByRole("dialog").evaluate((node) => { const box = node.getBoundingClientRect(); return box.left >= 0 && box.right <= innerWidth; }));
  await page.keyboard.press("Escape");
  await page.evaluate(() => { window.conversionTest.setLanguage("en-US"); delete document.documentElement.dataset.theme; });
  await page.setViewportSize({ width: 1280, height: 1000 });
}
async function checkFreshSourceRetry() {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await openPicker("codex");
  await checkBox("Provider Alpha");
  await page.evaluate(() => window.conversionTest.changeSource("codex-source"));
  await page.getByRole("button", { name: "Convert one by one (1)", exact: true }).click();
  await page.getByRole("alert").filter({ hasText: "The source provider changed" }).waitFor();
  let prepares = await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_provider_conversion"));
  check(prepares[0].args.request.expectedSourceFileHash === "alpha-revision");
  await page.evaluate(() => window.conversionTest.failNextRead());
  await page.getByRole("button", { name: "Reload source and retry", exact: true }).click();
  await page.getByRole("alert").filter({ hasText: "Source read failed" }).waitFor();
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_provider_conversion").length)) === 1);
  await page.getByRole("button", { name: "Reload source and retry", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.phase === "editing");
  prepares = await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_provider_conversion"));
  check(prepares.at(-1).args.request.expectedSourceFileHash === "codex-source-updated-revision");
  await page.getByRole("button", { name: "End conversion", exact: true }).click();
  await openPicker("codex");
  await checkBox("Provider Gamma");
  await page.evaluate(() => window.conversionTest.deleteSource("codex-second"));
  await page.getByRole("button", { name: "Convert one by one (1)", exact: true }).click();
  await page.getByRole("alert").filter({ hasText: "The source provider was not found" }).waitFor();
  const before = await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_provider_conversion").length);
  await page.getByRole("button", { name: "Reload source and retry", exact: true }).click();
  await page.waitForFunction(() => !window.conversionTest.snapshot().busy);
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_provider_conversion").length)) === before);
  check((await snapshot()).batch.phase === "error" && (await snapshot()).created.length === 0);
  await page.getByRole("button", { name: "End conversion", exact: true }).click();
}
async function checkBatch() {
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  check(await page.locator(".asb-row-item [data-provider-conversion-app]").count() === 0);
  await openPicker("codex");
  check(await page.getByRole("button", { name: "Convert one by one (0)", exact: true }).isDisabled());
  check(await page.getByRole("checkbox", { name: "Provider Beta", exact: true }).count() === 0);
  await checkBox("Select all");
  await page.getByRole("button", { name: "Convert one by one (2)", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.phase === "editing");
  const firstIdentity = (await snapshot()).session.profileId;
  check(!await page.getByRole("checkbox", { name: "Copy usage query with manual refresh only", exact: true }).isChecked());
  await checkBox("Copy usage query with manual refresh only");
  await page.evaluate(() => window.conversionTest.failNextSave());
  await page.getByRole("button", { name: "Add to Claude", exact: true }).click();
  await page.getByRole("alert").filter({ hasText: "Isolated save failure" }).waitFor();
  check((await snapshot()).batch.index === 0 && (await snapshot()).session.profileId === firstIdentity);
  const before = await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_profile_save").length);
  await page.locator("form").evaluate((form) => { form.requestSubmit(); form.requestSubmit(); });
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.index === 1 && window.conversionTest.snapshot().batch?.phase === "editing");
  let result = await snapshot();
  check(result.created.length === 1 && result.created[0].profile.id === firstIdentity);
  check(result.session.profileId !== firstIdentity && result.session.conversion.source.id === "codex-second");
  check(!await page.getByRole("checkbox", { name: "Copy usage query with manual refresh only", exact: true }).isChecked());
  check((await page.evaluate(() => window.conversionTest.calls.filter(({ command }) => command === "prepare_profile_save").length)) === before + 1);
  await page.getByRole("button", { name: "Add to Claude", exact: true }).click();
  await page.getByRole("dialog", { name: "Selected providers processed", exact: true }).waitFor();
  result = await snapshot();
  check(result.batch.saved === 2 && result.batch.skipped === 0 && result.created.length === 2);
  check(result.created[0].profile.apiKey === "fixture-alpha-key" && result.created[1].profile.apiKey === "fixture-gamma-key");
  check(result.created[0].profile.usageQuery.refreshIntervalMinutes === 0 && result.created[1].profile.usageQuery === null);
  check(result.sourcesUnchanged && result.target === null);
  await closeResult();
}
async function checkReverseAndSkip() {
  await convert("claude", ["Provider Beta", "Official login"]);
  await page.getByRole("tab", { name: "Models & routing", exact: true }).click();
  const contextLimit = page.getByRole("spinbutton", { name: "vendor-beta context window", exact: true });
  const outputLimit = page.getByRole("spinbutton", { name: "vendor-beta output limit", exact: true });
  check(await contextLimit.inputValue() === "" && await outputLimit.inputValue() === "");
  await page.getByRole("button", { name: "Add to Codex", exact: true }).click();
  check(!(await snapshot()).created.some(({ profile }) => profile.app === "codex"));
  await completeCodexLimits();
  await page.getByRole("button", { name: "Add to Codex", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.phase === "blocked");
  check((await page.getByRole("dialog").innerText()).includes("Official login cannot be converted"));
  await page.getByRole("button", { name: "Skip this provider", exact: true }).click();
  await page.getByRole("dialog", { name: "Selected providers processed", exact: true }).waitFor();
  const result = await snapshot();
  check(result.batch.saved === 1 && result.batch.skipped === 1 && result.sourcesUnchanged);
  check(result.created.at(-1).profile.catalog[0].contextWindow === 32768);
  await closeResult();
}
async function checkRetryAndCancel() {
  await page.evaluate(() => window.conversionTest.failNextPrepare());
  await convert("codex", ["Provider Alpha", "Provider Gamma"]);
  await page.getByRole("alert").filter({ hasText: "The source provider changed" }).waitFor();
  await page.getByRole("button", { name: "Reload source and retry", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.phase === "editing");
  await page.getByRole("button", { name: "Skip this provider", exact: true }).evaluate((button) => { button.click(); button.click(); });
  await page.waitForFunction(() => window.conversionTest.snapshot().batch?.index === 1 && window.conversionTest.snapshot().batch?.phase === "editing");
  check((await snapshot()).batch.skipped === 1);
  const before = (await snapshot()).created.length;
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  check((await snapshot()).batch === null && (await snapshot()).created.length === before);
  check((await snapshot()).app === "codex");
  check(await toolbar().evaluate((node) => node === document.activeElement));
  await page.evaluate(() => window.conversionTest.delayNext());
  await openPicker("codex");
  await checkBox("Provider Alpha");
  await page.getByRole("button", { name: "Convert one by one (1)", exact: true }).click();
  await page.waitForFunction(() => window.conversionTest.snapshot().waiting);
  await page.evaluate(() => { window.conversionTest.newClaude(); window.conversionTest.release(); });
  await page.waitForFunction(() => !window.conversionTest.snapshot().busy);
  check((await snapshot()).session.conversion === undefined && (await snapshot()).batch === null);
}
try {
  await checkBatch();
  await checkReverseAndSkip();
  await checkRetryAndCancel();
  await checkLateSave("prepare_profile_save", "codex", false);
  await checkLateSave("commit_profile_save", "codex", true);
  await checkLateSave("create_codex_profile", "claude", true);
  await checkSaveWhileAway();
  await checkSaveCannotBeSkipped("codex");
  await checkSaveCannotBeSkipped("claude");
  await checkFreshSourceRetry();
  await checkInvalidResult();
  await checkPickerPresentation();
  await checkTouchControls();
  const commands = await page.evaluate(() => window.conversionTest.calls);
  check(!commands.some(({ command }) => /execute_switch|query.*usage|fetch_provider_models|test_provider/.test(command)));
  check(blockedRequests.length === 0 && errors.length === 0);
  await fs.writeFile(path.join(output, "evidence.json"), `${JSON.stringify({ passed: true, assertions,
    realProviderRequests: 0, nativeConfigurationWrites: 0, errors, blockedRequests }, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ passed: true, assertions, output })}\n`);
} catch (error) {
  process.stderr.write(`UI assertion failed: ${error.stack ?? error.message}\n`);
  const [state, calls, text] = await Promise.all([
    snapshot().catch((capture) => ({ captureError: capture.message })),
    page.evaluate(() => window.conversionTest?.calls).catch((capture) => ({ captureError: capture.message })),
    page.locator("body").innerText().catch((capture) => `Capture failed: ${capture.message}`),
  ]);
  await fs.writeFile(path.join(output, "failure.json"), `${JSON.stringify({ error: error.message, errors,
    snapshot: state, calls, text }, null, 2)}\n`);
  await page.screenshot({ path: path.join(output, "failure.png"), fullPage: true }).catch((capture) =>
    process.stderr.write(`Screenshot capture failed: ${capture.message}\n`));
  throw error;
} finally {
  await context.close();
  await new Promise((resolve) => server.close(resolve));
}
