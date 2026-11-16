import assert from "node:assert/strict";
import fs from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";
import react from "@vitejs/plugin-react";

const root = fileURLToPath(new URL("../", import.meta.url));
await fs.mkdir(path.join(root, ".tmp"), { recursive: true });
const output = await fs.mkdtemp(path.join(root, ".tmp", "page-recovery-"));
const { chromium } = await import(process.env.ASB_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.ASB_PLAYWRIGHT_MODULE).href : "playwright-core");
await build({ root, configFile: false, plugins: [react()], logLevel: "error",
  resolve: { alias: { "@": path.join(root, "src") } },
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: { lib: { entry: path.join(root, "scripts/page-recovery-ui.fixture.tsx"), formats: ["es"], fileName: "fixture" },
    outDir: output, emptyOutDir: false, minify: false } });
const html = '<!doctype html><meta charset="utf-8"><div id="root"></div><script type="module" src="/fixture.js"></script>';
const server = http.createServer(async (request, response) => {
  if (request.url === "/") response.setHeader("Content-Type", "text/html; charset=utf-8").end(html);
  else if (request.url === "/fixture.js") {
    response.setHeader("Content-Type", "text/javascript; charset=utf-8").end(await fs.readFile(path.join(output, "fixture.js")));
  } else response.writeHead(404).end();
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
let browser;
const unexpectedErrors = [];
const blockedRequests = [];
let passed = false;
try {
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  await context.route("**/*", (route) => {
    if (new URL(route.request().url()).origin === origin) return route.continue();
    blockedRequests.push(route.request().url());
    return route.abort();
  });
  const page = await context.newPage();
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = { invoke() { throw new Error("Native commands are forbidden in this fixture"); } };
  });
  page.on("pageerror", (error) => {
    if (!error.message.includes("isolated-page-render-fixture")) unexpectedErrors.push(error.message);
  });
  await page.goto(origin);
  const nav = page.getByRole("navigation", { name: "Fixture navigation" });
  const tabs = page.getByRole("navigation", { name: "Nested navigation" });
  const healthy = page.getByRole("textbox", { name: "Healthy draft", exact: true });
  const tabDraft = page.getByRole("textbox", { name: "Healthy tab draft", exact: true });
  await healthy.fill("unsaved page draft");
  await tabDraft.fill("unsaved tab draft");
  await page.evaluate(() => { window.fixtureDocumentIdentity = {}; });
  const documentIdentity = await page.evaluateHandle(() => window.fixtureDocumentIdentity);
  await nav.getByText("Toggle failure", { exact: true }).click();
  await nav.getByText("Fault page", { exact: true }).click();
  await page.getByRole("button", { name: "Reopen this page", exact: true }).waitFor();
  await nav.getByText("Healthy page", { exact: true }).click();
  assert.equal(await healthy.inputValue(), "unsaved page draft");
  assert.equal(await tabDraft.inputValue(), "unsaved tab draft");
  await nav.getByText("Toggle failure", { exact: true }).click();
  await nav.getByText("Fault page", { exact: true }).click();
  await page.getByText("Recovered page content", { exact: true }).waitFor();
  await nav.getByText("Toggle failure", { exact: true }).click();
  await page.getByRole("button", { name: "Reopen this page", exact: true }).click();
  await page.getByRole("button", { name: "Reopen this page", exact: true }).waitFor();
  await nav.getByText("Toggle failure", { exact: true }).click();
  await page.getByRole("button", { name: "Reopen this page", exact: true }).click();
  await page.getByText("Recovered page content", { exact: true }).waitFor();
  await nav.getByText("Healthy page", { exact: true }).click();
  await nav.getByText("Toggle failure", { exact: true }).click();
  await tabs.getByText("Fault tab", { exact: true }).click();
  await page.getByRole("button", { name: "Reopen this page", exact: true }).waitFor();
  await tabs.getByText("Healthy tab", { exact: true }).click();
  assert.equal(await tabDraft.inputValue(), "unsaved tab draft");
  await nav.getByText("Toggle failure", { exact: true }).click();
  await tabs.getByText("Fault tab", { exact: true }).click();
  await page.getByText("Recovered tab content", { exact: true }).waitFor();
  await tabs.getByText("Healthy tab", { exact: true }).click();
  assert.equal(await healthy.inputValue(), "unsaved page draft");
  assert.equal(await page.evaluate((identity) => window.fixtureDocumentIdentity === identity, documentIdentity), true);
  const diagnostic = page.getByRole("region", { name: "Diagnostic fixture" });
  for (const [button, label] of [["Chinese", "上游重试指引（Retry-After）"], ["English", "Upstream retry guidance (Retry-After)"]]) {
    await nav.getByText(button, { exact: true }).click();
    const row = diagnostic.locator("dl > div").filter({ has: page.getByText(label, { exact: true }) });
    for (const value of ["120", "0", "Wed, 21 Oct 2026 07:28:00 GMT"]) {
      await diagnostic.getByRole("combobox").selectOption(value);
      assert.equal(await row.locator("dd").textContent(), value);
    }
    await diagnostic.getByRole("combobox").selectOption("absent");
    assert.equal(await row.count(), 0);
  }
  assert.deepEqual(unexpectedErrors, []);
  assert.deepEqual(blockedRequests, []);
  passed = true;
} finally {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
  await fs.writeFile(path.join(output, "result.json"), JSON.stringify({ passed, unexpectedErrors, blockedRequests }, null, 2));
}
console.log(JSON.stringify({ passed, evidence: path.join(output, "result.json") }));
