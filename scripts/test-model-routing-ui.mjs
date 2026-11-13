import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";
import http from "node:http";
import react from "@vitejs/plugin-react";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = path.join(root, ".tmp", "model-routing-ui");
const dist = path.join(output, "dist");
await fs.mkdir(output, { recursive: true });
const { chromium } = await import(process.env.ASB_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.ASB_PLAYWRIGHT_MODULE).href : "playwright-core");
await build({ root, configFile: false, mode: "test", plugins: [react()], logLevel: "error",
  resolve: { alias: { "@": path.join(root, "src") } },
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: { lib: { entry: path.join(root, "scripts/model-routing-ui.fixture.tsx"), formats: ["es"], fileName: "fixture" },
    outDir: dist, emptyOutDir: false, minify: false } });
const css = (await fs.readdir(dist)).find((name) => name.endsWith(".css"));
assert.ok(css, "The isolated build must produce its stylesheet");
await fs.mkdir(path.join(dist, "assets"), { recursive: true });
await fs.copyFile(path.join(root, "src", "assets", "routing-hub.png"), path.join(dist, "assets", "routing-hub.png"));
const html = `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/${css}"></head><body><div id="root"></div><script type="module" src="/fixture.js"></script></body></html>`;
const mime = { ".css": "text/css", ".js": "text/javascript", ".png": "image/png", ".webp": "image/webp",
  ".jpg": "image/jpeg", ".svg": "image/svg+xml", ".woff2": "font/woff2" };

const server = http.createServer(async (request, response) => {
  let name;
  try { name = decodeURIComponent(new URL(request.url, "http://localhost").pathname).slice(1); }
  catch { response.writeHead(400).end(); return; }
  if (name === "favicon.ico") { response.writeHead(204).end(); return; }
  if (!name || name === "index.html") { response.setHeader("Content-Type", "text/html; charset=utf-8"); response.end(html); return; }
  const target = path.resolve(dist, name);
  const relative = path.relative(dist, target);
  const contentType = mime[path.extname(target)];
  if (!relative || relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative) || !contentType) {
    response.writeHead(404).end(); return;
  }
  try {
    const content = await fs.readFile(target);
    response.setHeader("Content-Type", contentType);
    response.end(content);
  } catch { response.writeHead(404).end(); }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browserProfile = await fs.mkdtemp(path.join(os.tmpdir(), "asb-routing-ui-"));
const errors = [];
const blockedRequests = [];
let context;
let page;
let assertions = 0;
const check = (value, message) => { assert.ok(value, message); assertions++; };

async function openScenario(client = "codex", scenario = "") {
  await page.goto(`${origin}/?${new URLSearchParams({ client, scenario })}`, { waitUntil: "domcontentloaded" });
  await page.getByRole("combobox", { name: "Provider plan to view", exact: true }).waitFor();
}

async function chooseProvider(name) {
  await page.getByRole("combobox", { name: "Provider plan to view", exact: true }).click();
  await page.getByRole("option", { name, exact: true }).click();
  await page.getByRole("combobox", { name: "Provider plan to view", exact: true }).filter({ hasText: name }).waitFor();
}

function purpose(name) {
  return page.locator(".asb-route-overview-purpose").filter({ has: page.getByText(name, { exact: true }) });
}

async function noWrites() {
  check((await page.evaluate(() => window.routeTest.calls)).length === 0, "The overview must not invoke any backend command");
  check(await page.getByRole("textbox").count() === 0, "The overview must not expose editing fields");
}

async function unchangedActive() {
  const { actual, expected } = await page.evaluate(() => ({ actual: window.routeTest.snapshot().activeIds, expected: window.routeTest.initialActiveIds }));
  assert.deepEqual(actual, expected); assertions++;
}

async function codexOverview() {
  await openScenario();
  const before = await page.evaluate(() => window.routeTest.snapshot());
  check(await page.locator(".asb-route-overview-purpose").count() === 2);
  check((await purpose("Default subagent").innerText()).includes("Codex B"));
  check((await purpose("Default subagent").innerText()).includes("Fixed provider"));
  await chooseProvider("Codex B");
  await unchangedActive();
  check((await page.locator(".asb-route-overview-identity").innerText()).includes("Codex A"));
  check((await page.locator(".asb-route-overview-saved").innerText()).includes("inactive"));
  await purpose("Main model").getByRole("button").click();
  await purpose("Default subagent").getByRole("button").click();
  let data = await page.evaluate(() => ({ entries: window.routeTest.entries, ids: window.routeTest.ids }));
  assert.deepEqual(data.entries, [{ app: "codex", profileId: data.ids.codexB, role: "main" },
    { app: "codex", profileId: data.ids.codexB, role: "subagent" }]); assertions++;
  await page.getByRole("button", { name: "View active plan", exact: true }).click();
  await purpose("Default subagent").getByRole("button").click();
  data = await page.evaluate(() => ({ entries: window.routeTest.entries, ids: window.routeTest.ids }));
  assert.deepEqual(data.entries.at(-1), { app: "codex", profileId: data.ids.codexA, role: "subagent" }); assertions++;
  assert.deepEqual(await page.evaluate(() => window.routeTest.snapshot()), before); assertions++;
  await noWrites();
}

async function claudeOverview() {
  await openScenario("claude");
  check(await page.locator(".asb-route-overview-purpose").count() === 6);
  const automatic = await purpose("Haiku tier").innerText();
  check(automatic.includes("Automatic") && !automatic.includes("Fixed provider"));
  const primary = await purpose("Sonnet tier").innerText();
  check(primary.includes("Claude A") && primary.includes("sonnet-local") && primary.includes("Follow main connection"));
  const fixed = await purpose("Opus tier").innerText();
  const inherited = await purpose("Fable tier").innerText();
  check(fixed.includes("Claude B") && fixed.includes("shared-model") && fixed.includes("Fixed provider") && fixed.includes("1M"));
  check(inherited.includes("Claude B") && inherited.includes("shared-model") && inherited.includes("Fixed provider") && inherited.includes("1M"));
  check((await purpose("Main model").innerText()).includes("1M"));
  const self = await purpose("Default subagent").innerText();
  check(self.includes("Claude A") && self.includes("local-subagent") && self.includes("Fixed provider"));
  await purpose("Fable tier").getByRole("button").click();
  const data = await page.evaluate(() => ({ entries: window.routeTest.entries, ids: window.routeTest.ids }));
  assert.deepEqual(data.entries, [{ app: "claude", profileId: data.ids.claudeA, role: "fable" }]); assertions++;
  await chooseProvider("Claude B");
  await unchangedActive();
  await page.getByRole("button", { name: "Manage providers", exact: true }).click();
  assert.deepEqual(await page.evaluate(() => window.routeTest.providerEntries), ["claude"]); assertions++;
  await noWrites();
}

async function emptyAndMissingPlans() {
  await openScenario("codex", "empty");
  check(await page.getByRole("combobox", { name: "Provider plan to view", exact: true }).isDisabled());
  check((await page.locator(".asb-route-overview-empty").innerText()).includes("No plan is selected"));
  await page.getByRole("button", { name: "Manage providers", exact: true }).first().click();
  assert.deepEqual(await page.evaluate(() => window.routeTest.providerEntries), ["codex"]); assertions++;
  await noWrites();
  for (const client of ["codex", "claude"]) {
    await openScenario(client, "missing");
    check((await page.locator(".asb-route-overview-list").innerText()).includes("Target provider unavailable"));
    check(await page.locator(".asb-warn-text[role=status]").count() > 0, "Missing targets must retain an actionable warning");
    await unchangedActive();
    await noWrites();
  }
  await openScenario("claude");
  await chooseProvider("Claude B");
  await page.evaluate(() => window.routeTest.removeProvider(window.routeTest.ids.claudeB));
  await page.getByText("The previously viewed profile no longer exists. Select another provider.", { exact: true }).waitFor();
  check(await page.locator(".asb-route-overview-purpose").count() === 0, "Deleting the selected plan must not silently select another plan");
  await unchangedActive();
  await chooseProvider("Claude A");
  check(await page.locator(".asb-route-overview-purpose").count() === 6);
  await noWrites();
}

async function displayEvidence() {
  for (const client of ["codex", "claude"]) {
    await openScenario(client);
    await page.setViewportSize({ width: 1440, height: 1050 });
    await page.locator(".asb-route-overview-art").evaluate((image) => image.decode());
    check(await page.locator(".asb-route-overview-art").evaluate((image) => image.complete && image.naturalWidth > 0));
    for (const { language, width } of [{ language: "en-US", width: 1440 }, { language: "zh-CN", width: 640 }]) {
      await page.setViewportSize({ width, height: 1050 });
      await page.evaluate((language) => window.routeTest.setLanguage(language), language);
      await page.waitForFunction((language) => document.documentElement.lang === language, language);
      await page.getByRole("heading", { name: language === "zh-CN" ? "模型路由与网关" : "Model routing & gateway", exact: true }).waitFor();
      for (const theme of ["light", "dark"]) {
        await page.evaluate((theme) => { document.documentElement.dataset.theme = theme; }, theme);
        check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), `${client}/${language}/${theme}/${width} must fit`);
        await page.screenshot({ path: path.join(output, `${client}-overview-${language}-${theme}-${width}.png`), fullPage: true });
      }
    }
    await page.setViewportSize({ width: 360, height: 1050 });
    check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), "Small windows must reflow");
    check(!await page.locator(".asb-route-overview-art").isVisible(), "Decoration must yield to controls in small windows");
    await noWrites();
  }
}

try {
  context = await chromium.launchPersistentContext(browserProfile, {
    headless: true, args: ["--no-proxy-server", "--disable-gpu", "--no-sandbox"], timeout: 30000,
    viewport: { width: 1440, height: 1050 }, executablePath: process.env.ASB_BROWSER_EXECUTABLE });
  await context.route("**/*", async (route) => {
    if (new URL(route.request().url()).origin === origin) await route.continue();
    else { blockedRequests.push(route.request().url()); await route.abort(); }
  });
  page = context.pages()[0] ?? await context.newPage();
  page.setDefaultTimeout(15000);
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
  const asset = await context.request.get(`${origin}/assets/routing-hub.png`);
  check(asset.ok() && asset.headers()["content-type"] === "image/png", "The fixture server must serve nested PNG assets correctly");
  await codexOverview();
  await claudeOverview();
  await emptyAndMissingPlans();
  await displayEvidence();
  assert.deepEqual(errors, []); assertions++;
  assert.deepEqual(blockedRequests, []); assertions++;
  const result = { assertions, passed: true, errors, blockedRequests, evidence: output,
    coverage: ["saved plans never invoke IPC", "inactive selection preserves active identity and records", "purpose navigation uses plan owner UUID and role",
      "provider management receives selected client", "Claude automatic/Primary/fixed-self/fixed-target rules", "Fable inherits Opus destination and 1M",
      "missing targets remain explicit", "deleted selected plan is not silently replaced", "empty plan directs to providers",
      "decorative image and nested PNG serving", "both clients in English-wide and Chinese-narrow light/dark", "360px reflow"] };
  await fs.writeFile(path.join(output, "evidence.json"), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} finally {
  await context?.close();
  await new Promise((resolve) => server.close(resolve));
  const resolvedProfile = path.resolve(browserProfile);
  if (path.dirname(resolvedProfile) === path.resolve(os.tmpdir()) && path.basename(resolvedProfile).startsWith("asb-routing-ui-")) {
    await fs.rm(resolvedProfile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
}
