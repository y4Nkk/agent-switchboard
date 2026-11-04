import assert from "node:assert/strict";
import test from "node:test";
import { sameSettingValue } from "../src/lib/setting-values.ts";
import { withModelPreference, withoutModelPreferences } from "../src/components/provider-editor/claude-model-preferences.ts";

test("settings equality compares structured approvals by content and preserves list order", () => {
  assert.equal(sameSettingValue(
    { mode: "explicit", value: { granular: { rules: false, sandbox_approval: true, mcp_elicitations: true } } },
    { mode: "explicit", value: { granular: { mcp_elicitations: true, rules: false, sandbox_approval: true } } },
  ), true);
  assert.equal(sameSettingValue({ mode: "explicit", value: ["Read", "Write"] }, { mode: "explicit", value: ["Write", "Read"] }), false);
  assert.equal(sameSettingValue({ mode: "explicit", value: ["Read"] }, { mode: "explicit", value: ["Read"] }), true);
  assert.equal(sameSettingValue({ mode: "automatic" }, { mode: "explicit", value: [] }), false);
  assert.equal(sameSettingValue({ mode: "automatic" }, { mode: "explicit", value: "" }), false);
});

test("per-model fields preserve other native fields, with distinct inherited and auto windows", () => {
  const source = { modelSettings: { "model/~.id": { effortLevel: "high", toolSearch: true } }, effortLevel: "low" };
  const capped = withModelPreference(source, "model/~.id", "maxEffortLevel", "max");
  const automatic = withModelPreference(capped, "model/~.id", "autoCompactWindow", "auto");
  assert.deepEqual(automatic, { modelSettings: { "model/~.id": {
    effortLevel: "high", maxEffortLevel: "max", autoCompactWindow: "auto", toolSearch: true,
  } }, effortLevel: "low" });
  assert.deepEqual(withModelPreference(automatic, "model/~.id", "autoCompactWindow", undefined), capped);
  assert.deepEqual(withoutModelPreferences(automatic), { modelSettings: { "model/~.id": { toolSearch: true } }, effortLevel: "low" });
  assert.deepEqual(source.modelSettings["model/~.id"], { effortLevel: "high", toolSearch: true });
});

test("resetting modeled fields prunes only empty model containers", () => {
  assert.deepEqual(withoutModelPreferences({ modelSettings: {
    one: { autoCompactWindow: 200000 }, two: { maxEffortLevel: "low" }, three: { effortLevel: "xhigh" },
  }, host: true }), { host: true });
});

test("model identifiers remain literal object keys", () => {
  const result = withModelPreference({}, "__proto__", "effortLevel", "high");
  assert.equal(Object.hasOwn(result.modelSettings as object, "__proto__"), true);
  assert.deepEqual(JSON.parse(JSON.stringify(result)), JSON.parse('{"modelSettings":{"__proto__":{"effortLevel":"high"}}}'));
});
