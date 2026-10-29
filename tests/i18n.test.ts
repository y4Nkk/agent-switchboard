import assert from "node:assert/strict";
import test from "node:test";
import { applyLanguagePreference, currentLanguage, subscribeLanguage, tr } from "../src/i18n/current.ts";
import { commandErrorText, errorText, uiMessage } from "../src/i18n/errors.ts";
import { messages } from "../src/i18n/messages.ts";
import { resolveLanguage } from "../src/i18n/resolve.ts";
import { officialQuotaReadings, officialQuotaWindowName } from "../src/lib/usage-format.ts";
import { timeLabel } from "../src/lib/time.ts";

test("sample timestamps show seconds while ordinary timestamps keep minute precision", () => {
  const previous = currentLanguage();
  const iso = new Date(2026, 9, 26, 17, 50, 3).toISOString();
  try {
    applyLanguagePreference("zh-CN");
    assert.ok(timeLabel(iso).endsWith("17：50"));
    assert.ok(timeLabel(iso, "second").endsWith("17：50：03"));
    applyLanguagePreference("en-US");
    assert.ok(timeLabel(iso).endsWith("17:50"));
    assert.ok(timeLabel(iso, "second").endsWith("17:50:03"));
  } finally { applyLanguagePreference(previous); }
});

test("committed preference and system events publish one synchronous language snapshot", () => {
  const names = ["window", "document", "navigator"] as const;
  const original = names.map((name) => Object.getOwnPropertyDescriptor(globalThis, name));
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), { documentElement: { lang: "" } });
  let locales = ["en-US"];
  const navigator = { get languages() { return locales; } };
  for (const [name, value] of Object.entries({ window, document, navigator })) {
    Object.defineProperty(globalThis, name, { configurable: true, value });
  }
  const observed: string[] = [];
  let unsubscribe = () => {};
  try {
    applyLanguagePreference("en-US");
    unsubscribe = subscribeLanguage(() => observed.push(`${currentLanguage()}:${tr("format.usage.remaining")}`));
    applyLanguagePreference("zh-CN");
    assert.deepEqual(observed, ["zh-CN:剩余"]);
    assert.equal(document.documentElement.lang, "zh-CN");
    window.dispatchEvent(new Event("languagechange"));
    assert.equal(currentLanguage(), "zh-CN");
    applyLanguagePreference("system");
    assert.equal(currentLanguage(), "en-US");
    locales = ["zh-TW", "en-US"];
    window.dispatchEvent(new Event("languagechange"));
    assert.equal(currentLanguage(), "zh-CN");
    assert.equal(observed.at(-1), "zh-CN:剩余");
    unsubscribe();
    const count = observed.length;
    applyLanguagePreference("en-US");
    assert.equal(observed.length, count);
  } finally {
    unsubscribe();
    names.forEach((name, index) => {
      const descriptor = original[index];
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else Reflect.deleteProperty(globalThis, name);
    });
    applyLanguagePreference("system");
  }
});

test("system preference follows the primary desktop locale", () => {
  assert.equal(resolveLanguage("system", ["fr-FR", "zh-CN"]), "en-US");
  assert.equal(resolveLanguage("system", ["zh-TW", "en-US"]), "zh-CN");
  assert.equal(resolveLanguage("en-US", ["zh-CN"]), "en-US");
  assert.equal(resolveLanguage("system", []), "en-US");
});

test("retained command errors resolve nested stage and recovery keys in either language", () => {
  const error = { code: "commit-failed", message: "original diagnostic",
    messageKey: "errors.switch.commitFailed",
    params: { stageKey: "errors.switch.stage.backup", recoveryKey: "errors.switch.recoveryRestored" } };
  try {
    applyLanguagePreference("en-US");
    assert.match(commandErrorText(error, tr), /creating the backup/);
    assert.match(errorText(error, tr), /previous backup was restored/);
    assert.doesNotMatch(errorText(error, tr), /errors\.switch\./);
    const validation = uiMessage("codex.connection.headersMustBeObject");
    const english = errorText(validation, tr);
    const fieldNotice = uiMessage("mcp.error.slotNameRequired", { labelKey: "mcp.error.labelEnv" });
    assert.doesNotMatch(errorText(fieldNotice, tr), /mcp\.error\./);
    applyLanguagePreference("zh-CN");
    assert.match(commandErrorText(error, tr), /创建备份/);
    assert.notEqual(errorText(validation, tr), english);
    assert.equal(error.message, "original diagnostic");
    assert.equal(errorText(new Error("upstream diagnostic"), tr), "upstream diagnostic");
  } finally { applyLanguagePreference("system"); }
});

test("official window display covers duration units without changing source identities", () => {
  const quota = { status: "available" as const, at: null, stale: false, lastReset: null,
    windows: [{ label: "5 小时", usedPercent: 28, resetsAt: null },
      { label: "7 天", usedPercent: 54, resetsAt: null }] };
  try {
    applyLanguagePreference("en-US");
    assert.deepEqual(officialQuotaReadings(quota).map((row) => row.planName), ["5 hours", "7 days"]);
    assert.equal(officialQuotaWindowName("30 天"), "30 days");
    assert.equal(officialQuotaWindowName("1 小时"), "1 hour");
    assert.equal(officialQuotaWindowName("90 分钟"), "90 minutes");
    assert.deepEqual(quota.windows.map((window) => window.label), ["5 小时", "7 天"]);
    applyLanguagePreference("zh-CN");
    assert.equal(officialQuotaReadings(quota)[0].planName, "5 小时");
  } finally { applyLanguagePreference("system"); }
});

test("every catalog entry has identical interpolation parameters in both languages", () => {
  const params = (text: string) => [...new Set([...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]))].sort();
  for (const [key, [zh, en]] of Object.entries(messages)) {
    assert.ok(zh.trim() && en.trim(), key);
    assert.deepEqual(params(zh), params(en), key);
  }
});
