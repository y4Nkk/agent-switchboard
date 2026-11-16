import assert from "node:assert/strict";
import { messages } from "../src/i18n/messages.ts";
import { LEVEL_FILTERS, LOG_LEVEL_OPTIONS, RUNTIME_LOG_ACTION_LABELS,
  RUNTIME_LOG_LEVEL_LABELS } from "../src/lib/runtime-log-labels.ts";
import { providerDiagnostic } from "./fixtures/provider-diagnostic.ts";

interface NativeContract {
  runtimeActions: string[];
  runtimeLevels: string[];
  runtimeSeverities: string[];
  settings: unknown;
  diagnostic: unknown;
}

function exactValues(actual: string[], expected: string[], context: string) {
  assert.equal(new Set(actual).size, actual.length, `${context}: duplicate frontend value`);
  assert.deepEqual([...actual].sort(), [...expected].sort(), context);
}

let checkedLabels = 0;
function label(key: string) {
  assert.ok(Object.hasOwn(messages, key), `Missing frontend message: ${key}`);
  const entry = messages[key as keyof typeof messages];
  assert.equal(entry.length, 2, key);
  for (const text of entry) {
    assert.ok(text.trim(), `Empty frontend message: ${key}`);
    assert.doesNotMatch(text, /\{\w+\}/, `Catalog labels cannot require interpolation: ${key}`);
  }
  checkedLabels++;
}

function catalog(value: unknown): void {
  if (typeof value === "string" && value.startsWith("ownership.")) label(value);
  else if (Array.isArray(value)) value.forEach(catalog);
  else if (value !== null && typeof value === "object") Object.values(value).forEach(catalog);
}

process.stdin.setEncoding("utf8");
let input = "";
for await (const chunk of process.stdin) input += chunk;
const native: NativeContract = JSON.parse(input);
exactValues(Object.keys(RUNTIME_LOG_ACTION_LABELS), native.runtimeActions, "runtime log actions");
exactValues(Object.keys(RUNTIME_LOG_LEVEL_LABELS), native.runtimeLevels, "runtime log levels");
exactValues(LOG_LEVEL_OPTIONS.map((option) => option.value), native.runtimeLevels, "recording levels");
exactValues(LEVEL_FILTERS.map((option) => option.value), ["all", ...native.runtimeSeverities], "severity filters");
Object.values(RUNTIME_LOG_ACTION_LABELS).forEach(label);
Object.values(RUNTIME_LOG_LEVEL_LABELS).forEach(label);
LEVEL_FILTERS.forEach((option) => label(option.labelKey));
const logLabels = checkedLabels;
catalog(native.settings);
assert.deepEqual(native.diagnostic, providerDiagnostic, "request diagnostic wire fields and fixture must match");
assert.ok(checkedLabels > logLabels, "settings labels must be checked");
console.log(`ok: native log values, diagnostic fields and ${checkedLabels} renderer labels match`);
