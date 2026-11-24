import assert from "node:assert/strict";
import { test } from "node:test";
import { switchClickOutcome } from "../src/tray/tray-switch.ts";

const armed = { providerId: "profile-a", expiresAt: 5000 };

test("the first click on a provider row only marks it as the switch target", () => {
  assert.equal(switchClickOutcome(null, "profile-a", 0), "arm");
});

test("clicking another row moves the mark instead of switching", () => {
  assert.equal(switchClickOutcome(armed, "profile-b", 500), "arm");
});

test("the second click on the marked row switches", () => {
  assert.equal(switchClickOutcome(armed, "profile-a", 4999), "switch");
});

test("the same row must be marked again after the deadline", () => {
  assert.equal(switchClickOutcome(armed, "profile-a", 5000), "arm");
});
