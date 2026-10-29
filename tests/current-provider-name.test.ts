import assert from "node:assert/strict";
import test from "node:test";
import type { ConfigFileStatus } from "../src/api/status.ts";
import { currentProviderName, currentProviderProfile } from "../src/lib/current-provider-name.ts";

const profiles = [{ app: "codex" as const, id: "official-fixture", name: "Saved official login", websiteUrl: null }];

function status(): ConfigFileStatus {
  return {
    app: "codex", path: "isolated-config", exists: true, syntaxOk: true,
    readError: null, recoveryIssue: null, clientSettings: null, clientSettingsError: null,
    activeProfileId: "official-fixture", lastSwitch: null,
    route: {
      app: "codex", routeMode: "official", providerName: "openai", model: "live-model",
      baseUrl: null, apiKey: "", wireApi: null, codexModelOptions: null,
      haikuModel: null, sonnetModel: null, opusModel: null, fableModel: null, availableModels: null, scopeWarnings: [],
    },
  };
}

test("current provider identity follows the observed connection while model and preferences differ", () => {
  const observed = status();
  observed.clientSettings = { settings: { "features.memories": { mode: "explicit", value: true } } };
  assert.equal(currentProviderName(observed, profiles), profiles[0].name);
  assert.equal(currentProviderProfile(observed, profiles), profiles[0]);
  assert.equal(observed.route?.model, "live-model");
});

test("write and restore records do not identify a provider for an unlinked connection", () => {
  const observed = status();
  observed.activeProfileId = null;
  for (const operation of ["projection", "restore"] as const) {
    observed.lastSwitch = {
      app: "codex", profileId: profiles[0].id, profileName: profiles[0].name,
      contentHash: "historical-hash", backupId: "isolated-backup", at: "2026-10-16T00:00:00Z", operation,
    };
    assert.equal(currentProviderProfile(observed, profiles), null);
    assert.equal(currentProviderName(observed, profiles), "openai");
  }
});

test("configuration diagnostics are independent of the recognized profile", () => {
  const observed = status();
  observed.clientSettingsError = "invalid setting";
  observed.recoveryIssue = "pending recovery";
  assert.equal(currentProviderName(observed, profiles), profiles[0].name);
  assert.equal(observed.clientSettingsError, "invalid setting");
  assert.equal(observed.recoveryIssue, "pending recovery");
});
