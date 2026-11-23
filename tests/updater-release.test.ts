import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { assertReleaseVersion, readReleaseNotes, readUpdateAdvisories } from "../scripts/updater-release.mjs";

async function withChangelog(content: string, check: (changelogFile: string) => Promise<void>) {
  const directory = await mkdtemp(path.join(tmpdir(), "asb-release-notes-"));
  try {
    const changelogFile = path.join(directory, "CHANGELOG.md");
    await writeFile(changelogFile, content, "utf8");
    await check(changelogFile);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

test("release notes carry the selected heading's date and only its own body", async () => {
  await withChangelog([
    "# 更新日志", "", "## [Unreleased]", "", "- 未发布内容", "",
    "## [1.2.3] - 2026-11-12", "", "### 改进", "", "- 本版改动", "",
    "## [1.2.2] - 2026-11-10", "", "- 旧版内容", "",
  ].join("\r\n"), async (changelogFile) => {
    assert.equal(await readReleaseNotes({ changelogFile, version: "1.2.3" }),
      "发布日期：2026-11-12\n\n### 改进\n\n- 本版改动");
  });
});

test("an undated release is rejected instead of using the current date or a body bullet", async () => {
  await withChangelog("## [1.2.3]\n\n- 2026-11-12\n", async (changelogFile) => {
    await assert.rejects(readReleaseNotes({ changelogFile, version: "1.2.3" }), /release date/);
  });
});

test("release dates require an exact ISO calendar date", async () => {
  for (const date of ["2026-02-30", "2026-13-01", "2026-1-2", "tomorrow"]) {
    await withChangelog(`## [1.2.3] - ${date}\n\n- 改动\n`, async (changelogFile) => {
      await assert.rejects(readReleaseNotes({ changelogFile, version: "1.2.3" }), /release date/);
    });
  }
});

test("a dated empty release cannot publish a date as its only release note", async () => {
  await withChangelog("## [1.2.3] - 2026-11-12\n\n## [1.2.2] - 2026-11-10\n\n- 旧版\n",
    async (changelogFile) => {
      await assert.rejects(readReleaseNotes({ changelogFile, version: "1.2.3" }), /are empty/);
    });
});

test("a missing version has a distinct error", async () => {
  await withChangelog("## [1.2.2] - 2026-11-10\n\n- 旧版\n", async (changelogFile) => {
    await assert.rejects(readReleaseNotes({ changelogFile, version: "1.2.3" }), /Missing release notes/);
  });
});

async function withWorkspace(check: (cargoManifest: string, writeLock: (versions: string[]) => Promise<void>) => Promise<void>) {
  const directory = await mkdtemp(path.join(tmpdir(), "asb-release-version-"));
  try {
    const members = ["crates/core", "crates/switch", "desktop"];
    const names = ["asb-core", "asb-switch", "agent-switchboard"];
    const cargoManifest = path.join(directory, "Cargo.toml");
    await writeFile(cargoManifest,
      `[workspace]\nmembers = ${JSON.stringify(members)}\n\n[workspace.package]\nversion = "1.2.3"\n`);
    for (const [index, member] of members.entries()) {
      await mkdir(path.join(directory, member), { recursive: true });
      await writeFile(path.join(directory, member, "Cargo.toml"),
        `[package]\nname = "${names[index]}"\nversion.workspace = true\n`);
    }
    const writeLock = async (versions: string[]) => {
      const packages = versions.map((version, index) =>
        `[[package]]\nname = "${names[index % names.length]}"\nversion = "${version}"\n`);
      await writeFile(path.join(directory, "Cargo.lock"), `version = 4\n\n${packages.join("\n")}`);
    };
    await writeLock(["1.2.3", "1.2.3", "1.2.3"]);
    await check(cargoManifest, writeLock);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

test("release version checks all inherited workspace packages in the lock file", async () => {
  await withWorkspace(async (cargoManifest) => {
    await assertReleaseVersion({ cargoManifest, tag: "v1.2.3", version: "1.2.3" });
    await assert.rejects(assertReleaseVersion({ cargoManifest, tag: "v1.2.2", version: "1.2.3" }), /tag/);
    await assert.rejects(assertReleaseVersion({ cargoManifest, tag: "v1.2.2", version: "1.2.2" }), /Cargo.toml/);
  });
});

test("an out-of-date workspace lock package is identified before the build", async () => {
  await withWorkspace(async (cargoManifest, writeLock) => {
    await writeLock(["1.2.2", "1.2.3", "1.2.3"]);
    await assert.rejects(assertReleaseVersion({ cargoManifest, tag: "v1.2.3", version: "1.2.3" }),
      /package asb-core has version 1\.2\.2; expected 1\.2\.3/);
  });
});

test("missing and duplicate local lock packages are rejected", async () => {
  await withWorkspace(async (cargoManifest, writeLock) => {
    await writeLock(["1.2.3", "1.2.3"]);
    await assert.rejects(assertReleaseVersion({ cargoManifest, tag: "v1.2.3", version: "1.2.3" }),
      /agent-switchboard; found 0/);
    await writeLock(["1.2.3", "1.2.3", "1.2.3", "1.2.3"]);
    await assert.rejects(assertReleaseVersion({ cargoManifest, tag: "v1.2.3", version: "1.2.3" }),
      /asb-core; found 2/);
  });
});

async function withCatalog(content: string, check: (catalogFile: string) => Promise<void>) {
  const directory = await mkdtemp(path.join(tmpdir(), "asb-release-advisories-"));
  try {
    const catalogFile = path.join(directory, "update-advisories.json");
    await writeFile(catalogFile, content, "utf8");
    await check(catalogFile);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

const RECORD = JSON.stringify({
  severity: "urgent",
  affected_from: "0.3.10",
  fixed_in: "0.3.13",
  reason_zh: "切换可能损坏配置",
  reason_en: "Switching may corrupt configuration",
});

test("valid advisory records pass through unchanged", async () => {
  const record = JSON.parse(RECORD);
  const records = [record, {
    ...record,
    affected_from: "18446744073709551614.0.0",
    fixed_in: "18446744073709551615.0.0",
  }];
  await withCatalog(JSON.stringify(records), async (catalogFile) => {
    assert.deepEqual(await readUpdateAdvisories(catalogFile), records);
  });
});

test("the checked-in catalog is a valid advisory list", async () => {
  const catalogFile = fileURLToPath(new URL("../release/update-advisories.json", import.meta.url));
  assert.deepEqual(await readUpdateAdvisories(catalogFile), []);
});

test("advisory catalog rejects anything but strict cumulative records", async () => {
  const cases: [string, RegExp][] = [
    ["{", /valid JSON/],
    ["{}", /must be an array/],
    [`[${RECORD}, null]`, /must be an object/],
    ['[["urgent","0.3.10","0.3.13","原因","reason"]]', /must be an object/],
  ];
  for (const [content, pattern] of cases) {
    await withCatalog(content, async (catalogFile) => {
      await assert.rejects(readUpdateAdvisories(catalogFile), pattern);
    });
  }
});

test("advisory records require the exact key set, known severity, ordered versions, and both reasons", async () => {
  const mutate = (patch: Record<string, unknown>, remove: string[] = []) => {
    const record = { ...JSON.parse(RECORD), ...patch };
    for (const key of remove) delete record[key];
    return JSON.stringify([record]);
  };
  const cases: [string, RegExp][] = [
    [mutate({ extra: 1 }), /exactly the keys/],
    [mutate({}, ["fixed_in"]), /exactly the keys/],
    [mutate({ severity: "critical" }), /severity must be important or urgent/],
    [mutate({ severity: { urgent: null } }), /severity must be important or urgent/],
    [mutate({ affected_from: "0.3.13", fixed_in: "0.3.13" }), /lower than fixed_in/],
    [mutate({ affected_from: "0.3.14", fixed_in: "0.3.13" }), /lower than fixed_in/],
    [mutate({ affected_from: "0.3.10-beta.1" }), /plain X.Y.Z/],
    [mutate({ fixed_in: "0.3.13+build" }), /plain X.Y.Z/],
    [mutate({ affected_from: "0.03.10" }), /plain X.Y.Z/],
    [mutate({ affected_from: ["0.3.10"] }), /plain X.Y.Z/],
    [mutate({ fixed_in: "0.3.13\n" }), /plain X.Y.Z/],
    [mutate({ fixed_in: "18446744073709551616.0.0" }), /plain X.Y.Z/],
    [mutate({ fixed_in: "0.3" }), /plain X.Y.Z/],
    [mutate({ reason_zh: " \u0085 " }), /non-empty zh-CN and en-US reasons/],
    [mutate({ reason_en: "" }), /non-empty zh-CN and en-US reasons/],
    [mutate({ reason_en: "\uD800" }), /valid Unicode/],
  ];
  for (const [content, pattern] of cases) {
    await withCatalog(content, async (catalogFile) => {
      await assert.rejects(readUpdateAdvisories(catalogFile), pattern);
    });
  }
});
