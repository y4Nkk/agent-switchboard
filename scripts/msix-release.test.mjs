import assert from "node:assert/strict";
import test from "node:test";

import {
  createMsixManifest,
  msixAssetName,
  toMsixVersion,
} from "./msix-release.mjs";

const identity = {
  name: "yy1.AgentSwitchboard",
  publisher: "CN=FFA766F3-961D-4013-91DD-95D1867EBC0F",
  publisherDisplayName: "yy1",
  displayName: "Agent Switchboard",
  description: "Manage Codex and Claude Code configuration.",
};

test("maps the Cargo version to the Store-required four-part version", () => {
  assert.equal(toMsixVersion("0.1.5"), "1.1.5.0");
  assert.equal(toMsixVersion("1.0.0"), "2.0.0.0");
  assert.throws(
    () => toMsixVersion("0.1.5-rc.1"),
    /stable major\.minor\.patch/,
  );
});

test("renders the reserved Store identity into the full-trust x64 manifest", () => {
  const manifest = createMsixManifest(identity, "0.1.5");

  assert.match(manifest, /Name="yy1\.AgentSwitchboard"/);
  assert.match(manifest, /Publisher="CN=FFA766F3-961D-4013-91DD-95D1867EBC0F"/);
  assert.match(manifest, /PublisherDisplayName>yy1<\/PublisherDisplayName>/);
  assert.match(manifest, /Version="1\.1\.5\.0"/);
  assert.match(manifest, /ProcessorArchitecture="x64"/);
  assert.match(manifest, /EntryPoint="Windows\.FullTrustApplication"/);
  assert.match(manifest, /<rescap:Capability Name="runFullTrust" \/>/);
});

test("keeps the GitHub artifact name on the Cargo release version", () => {
  assert.equal(
    msixAssetName("0.1.5"),
    "agent-switchboard_0.1.5_windows-x86_64.msix",
  );
});
