import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import { generateUpdaterManifest, readReleaseNotes, stageBuildArtifacts } from "./updater-release.mjs";

const VERSION = "0.1.2";
const TAG = `v${VERSION}`;
const REPOSITORY = "y4Nkk/agent-switchboard";
const execFileAsync = promisify(execFile);
const scriptPath = fileURLToPath(new URL("./updater-release.mjs", import.meta.url));

async function writeFixture(root, relativePath, contents) {
  const fixturePath = path.join(root, relativePath);
  await mkdir(path.dirname(fixturePath), { recursive: true });
  await writeFile(fixturePath, contents);
}

async function writeCargoManifest(root) {
  const cargoManifest = path.join(root, "Cargo.toml");
  await writeFile(cargoManifest, `[workspace]\n\n[workspace.package]\nversion = "${VERSION}"\n`);
  return cargoManifest;
}

async function writeAllUpdaterAssets(root) {
  const names = [
    `agent-switchboard_${VERSION}_windows-x86_64-nsis.exe`,
    `agent-switchboard_${VERSION}_darwin-aarch64.app.tar.gz`,
    `agent-switchboard_${VERSION}_darwin-x86_64.app.tar.gz`,
    `agent-switchboard_${VERSION}_linux-x86_64.deb`,
    `agent-switchboard_${VERSION}_linux-x86_64.AppImage`,
  ];
  for (const name of names) {
    await writeFixture(root, name, `payload:${name}`);
    await writeFixture(root, `${name}.sig`, `signature:${name}\n`);
  }
}

async function writeReleaseNotes(root, contents = "### 新功能\n\n- 更新说明\n") {
  const notesFile = path.join(root, "release-notes.md");
  await writeFile(notesFile, contents);
  return notesFile;
}

test("stageBuildArtifacts names distinct macOS updater payloads and preserves signatures", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-updater-stage-"));
  const bundleDirectory = path.join(root, "bundle");
  const outputDirectory = path.join(root, "assets");
  await writeFixture(bundleDirectory, "dmg/Agent Switchboard.dmg", "arm disk image");
  await writeFixture(bundleDirectory, "macos/Agent Switchboard.app.tar.gz", "arm payload");
  await writeFixture(bundleDirectory, "macos/Agent Switchboard.app.tar.gz.sig", "arm signature\n");

  await stageBuildArtifacts({
    build: "macos-arm64",
    bundleDirectory,
    includeUpdater: true,
    outputDirectory,
    version: VERSION,
  });

  assert.equal(
    await readFile(path.join(outputDirectory, `agent-switchboard_${VERSION}_darwin-aarch64.app.tar.gz`), "utf8"),
    "arm payload",
  );
  assert.equal(
    await readFile(path.join(outputDirectory, `agent-switchboard_${VERSION}_darwin-aarch64.app.tar.gz.sig`), "utf8"),
    "arm signature\n",
  );
});

test("Windows releases stage only the custom installer and its own signature", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-installer-stage-"));
  const bundleDirectory = path.join(root, "bundle");
  const outputDirectory = path.join(root, "assets");
  await writeFixture(bundleDirectory, "nsis/App-setup.exe", "internal engine");
  await writeFixture(bundleDirectory, "installer/App-setup.exe", "custom installer");
  await writeFixture(bundleDirectory, "installer/App-setup.exe.sig", "custom signature");
  await stageBuildArtifacts({ build: "windows-x64", bundleDirectory, outputDirectory, version: VERSION, includeUpdater: true });
  const artifact = path.join(outputDirectory, `agent-switchboard_${VERSION}_windows-x86_64-nsis.exe`);
  assert.equal(await readFile(artifact, "utf8"), "custom installer");
  assert.equal(await readFile(`${artifact}.sig`, "utf8"), "custom signature");
});

test("Windows releases reject a bare engine without the custom installer", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-installer-missing-"));
  await writeFixture(root, "bundle/nsis/App-setup.exe", "internal engine");
  await assert.rejects(stageBuildArtifacts({ build: "windows-x64", bundleDirectory: path.join(root, "bundle"), outputDirectory: path.join(root, "assets"), version: VERSION, includeUpdater: false }), /Missing bundle directory/);
});

test("generateUpdaterManifest maps every updater asset to the release URL and actual signature text", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-updater-manifest-"));
  const cargoManifest = await writeCargoManifest(root);
  await writeAllUpdaterAssets(root);
  const notesFile = await writeReleaseNotes(root);
  const outputFile = path.join(root, "latest.json");

  const manifest = await generateUpdaterManifest({
    cargoManifest,
    inputDirectory: root,
    notesFile,
    outputFile,
    pubDate: "2026-09-03T00:00:00.000Z",
    repository: REPOSITORY,
    tag: TAG,
    version: VERSION,
  });

  assert.deepEqual(Object.keys(manifest.platforms), [
    "windows-x86_64-nsis",
    "darwin-aarch64-app",
    "darwin-x86_64-app",
    "linux-x86_64-deb",
    "linux-x86_64-appimage",
  ]);
  assert.equal(
    manifest.platforms["darwin-aarch64-app"].signature,
    `signature:agent-switchboard_${VERSION}_darwin-aarch64.app.tar.gz\n`,
  );
  assert.equal(
    manifest.platforms["linux-x86_64-appimage"].url,
    `https://github.com/${REPOSITORY}/releases/download/${TAG}/agent-switchboard_${VERSION}_linux-x86_64.AppImage`,
  );
  assert.deepEqual(JSON.parse(await readFile(outputFile, "utf8")), manifest);
  assert.equal(manifest.notes, "### 新功能\n\n- 更新说明");
});

test("readReleaseNotes returns only the requested version body", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-release-notes-"));
  const changelogFile = path.join(root, "CHANGELOG.md");
  await writeFile(changelogFile, "# 更新日志\n\n## [0.1.2] - 2026-09-03\n\n### 新功能\n\n- 最新功能\n\n## [0.1.1] - 2026-09-01\n\n### 修复\n\n- 旧修复\n");

  await assert.doesNotReject(async () => {
    assert.equal(await readReleaseNotes({ changelogFile, version: VERSION }), "### 新功能\n\n- 最新功能");
  });
  await assert.rejects(readReleaseNotes({ changelogFile, version: "0.1.3" }), /Missing release notes/);
});

test("generateUpdaterManifest rejects empty release notes", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-updater-empty-notes-"));
  const cargoManifest = await writeCargoManifest(root);
  await writeAllUpdaterAssets(root);
  const notesFile = await writeReleaseNotes(root, "\n");

  await assert.rejects(
    generateUpdaterManifest({
      cargoManifest,
      inputDirectory: root,
      notesFile,
      outputFile: path.join(root, "latest.json"),
      repository: REPOSITORY,
      tag: TAG,
      version: VERSION,
    }),
    /Release notes must not be empty/,
  );
});

test("generateUpdaterManifest rejects a missing updater signature", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-updater-missing-signature-"));
  const cargoManifest = await writeCargoManifest(root);
  await writeAllUpdaterAssets(root);
  const notesFile = await writeReleaseNotes(root);
  await writeFixture(root, `agent-switchboard_${VERSION}_linux-x86_64.deb.sig`, "");

  await assert.rejects(
    generateUpdaterManifest({
      cargoManifest,
      inputDirectory: root,
      notesFile,
      outputFile: path.join(root, "latest.json"),
      repository: REPOSITORY,
      tag: TAG,
      version: VERSION,
    }),
    /Empty updater signature/,
  );
});

test("generateUpdaterManifest rejects a tag that diverges from the workspace version", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-updater-version-"));
  const cargoManifest = await writeCargoManifest(root);
  await writeAllUpdaterAssets(root);
  const notesFile = await writeReleaseNotes(root);

  await assert.rejects(
    generateUpdaterManifest({
      cargoManifest,
      inputDirectory: root,
      notesFile,
      outputFile: path.join(root, "latest.json"),
      repository: REPOSITORY,
      tag: "v9.9.9",
      version: VERSION,
    }),
    /must equal/,
  );
});

test("workspace-version runs when the release script is invoked by its absolute path", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "asb-updater-cli-"));
  const cargoManifest = await writeCargoManifest(root);

  const { stdout } = await execFileAsync(process.execPath, [
    scriptPath,
    "workspace-version",
    "--cargo-manifest",
    cargoManifest,
  ]);

  assert.equal(stdout, `${VERSION}\n`);
});
