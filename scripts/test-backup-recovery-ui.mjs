import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";
import react from "@vitejs/plugin-react";

const repository = fileURLToPath(new URL("../", import.meta.url));
const temporary = path.join(repository, ".tmp");
await fs.mkdir(temporary, { recursive: true });
const output = await fs.mkdtemp(path.join(temporary, "backup-recovery-ui-"));
const { chromium } = await import(process.env.ASB_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.ASB_PLAYWRIGHT_MODULE).href : "playwright-core");
await build({ root: repository, configFile: false, mode: "test", plugins: [react()], logLevel: "error",
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: { lib: { entry: path.join(repository, "scripts/backup-recovery-ui.fixture.tsx"),
    name: "BackupRecoveryFixture", formats: ["iife"], fileName: () => "fixture.js" },
  outDir: output, emptyOutDir: false } });

const context = await chromium.launchPersistentContext(path.join(output, "browser-profile"), {
  executablePath: process.env.ASB_BROWSER_EXECUTABLE, headless: true,
  args: ["--no-proxy-server", "--disable-gpu", "--disable-background-networking"],
});
try {
  await context.route("**/*", (route) => route.abort());
  const page = context.pages()[0];
  page.setDefaultTimeout(10000);
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.setContent('<!doctype html><html lang="en"><div id="root"></div></html>');
  await page.addScriptTag({ path: path.join(output, "fixture.js") });
  const transaction = page.getByRole("button", { name: "Restore transaction", exact: true });
  const dialog = page.getByRole("dialog");
  const restore = dialog.getByRole("button", { name: "Restore", exact: true });
  const calls = () => page.evaluate(() => window.backupRecoveryFixture.calls);
  const setFailure = (error) => page.evaluate((next) => window.backupRecoveryFixture.setFailure(next), error);
  const setReady = (ready) => page.evaluate((next) => window.backupRecoveryFixture.setReady(next), ready);

  await transaction.waitFor().catch(async (failure) => {
    console.error({ errors, view: await page.locator("body").innerText(),
      commandFailures: await page.evaluate(() => window.backupRecoveryFixture?.unexpected) });
    throw failure;
  });
  assert.ok(await page.getByRole("button", { name: "Restore", exact: true }).isDisabled());
  await setFailure({ code: "application-preparing", message: "A configuration check is already running." });
  await transaction.click();
  await restore.click();
  const alert = dialog.getByRole("alert");
  await alert.waitFor();
  assert.match(await alert.innerText(), /Restore failed.*configuration check is already running/);
  assert.deepEqual(await calls(), [{ backupId: "pending-backup", confirmWrite: true, transactionOnly: true }]);

  await setFailure(null);
  await restore.click();
  await dialog.waitFor({ state: "hidden" });
  assert.equal((await calls()).length, 2);

  await transaction.click();
  await setReady(true);
  await setFailure({ code: "transaction-backup-mismatch", message: "The transaction has already been recovered." });
  await restore.click();
  await alert.waitFor();
  assert.match(await alert.innerText(), /transaction has already been recovered/);
  assert.equal((await calls()).at(-1).transactionOnly, true);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await dialog.waitFor({ state: "hidden" });

  await setFailure(null);
  await page.getByRole("button", { name: "Restore", exact: true }).first().click();
  await restore.click();
  await dialog.waitFor({ state: "hidden" });
  assert.deepEqual((await calls()).at(-1), { backupId: "older-backup", confirmWrite: true, transactionOnly: false });
  assert.deepEqual(await page.evaluate(() => window.backupRecoveryFixture.unexpected), []);
  assert.deepEqual(errors, []);
  console.log("Backup recovery UI passed: degraded restore, visible failure, retry, stale confirmation, and ordinary restore.");
} finally {
  await context.close();
}
