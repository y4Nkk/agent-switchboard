import { createHash } from "node:crypto";
import { lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");

function inside(parent, candidate) {
  const suffix = relative(parent, candidate);
  return suffix !== "" && !suffix.startsWith(`..${sep}`) && suffix !== ".." && !isAbsolute(suffix);
}

function providerFiles(configuration) {
  return ["providers/codex", "providers/codex/official", "providers/claude"].flatMap((directory) => {
    const target = join(configuration, directory);
    let entries;
    try {
      const info = lstatSync(target);
      if (info.isSymbolicLink() || !info.isDirectory()) throw new Error("Provider fixture source must be a real directory");
      if (!inside(configuration, realpathSync(target))) throw new Error("Provider source directory escaped its configuration owner");
      entries = readdirSync(target, { withFileTypes: true });
    } catch (error) {
      if (error.code === "ENOENT") return [];
      throw error;
    }
    return entries.filter((entry) => entry.name.endsWith(".json")).map((entry) => {
      if (entry.isSymbolicLink() || !entry.isFile()) throw new Error("Provider fixture source must be a regular JSON file");
      return join(directory, entry.name);
    });
  }).sort();
}

function sourceConfiguration() {
  const { identifier } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
  if (!/^[a-z0-9.-]+$/i.test(identifier)) throw new Error("Application identifier is not a directory name");
  const appdata = process.env.APPDATA || join(homedir(), "AppData", "Roaming");
  if (process.platform !== "win32" || !isAbsolute(appdata)) {
    throw new Error("Real-provider acceptance requires the verified Windows application data owner");
  }
  const appRoot = realpathSync(join(appdata, identifier));
  const configuration = realpathSync(join(appRoot, "state", "configuration"));
  if (!inside(appRoot, configuration)) throw new Error("Provider source escaped the application directory");
  return configuration;
}

/** Copies only saved provider documents, before the caller isolates its child environment. */
export function prepareRealFixtures(sandbox) {
  const ignoredRoot = join(root, ".tmp");
  mkdirSync(ignoredRoot, { recursive: true });
  const canonicalIgnoredRoot = realpathSync(ignoredRoot);
  if (!inside(realpathSync(root), canonicalIgnoredRoot)) throw new Error("Ignored test directory escaped the workspace");
  mkdirSync(sandbox, { recursive: true });
  const canonicalSandbox = realpathSync(sandbox);
  if (!inside(canonicalIgnoredRoot, canonicalSandbox)) throw new Error("Provider copies require an isolated workspace sandbox");
  const source = sourceConfiguration();
  const files = providerFiles(source);
  if (!files.length) throw new Error("No real saved provider documents were found");
  const fixtureRoot = join(canonicalSandbox, "real-provider-fixtures");
  mkdirSync(fixtureRoot);
  const configuration = join(fixtureRoot, "configuration");
  const baseline = new Map();
  const categories = { codex: 0, claude: 0, official: 0 };
  const protocols = {};
  try {
    for (const file of files) {
      const bytes = readFileSync(join(source, file));
      const hash = digest(bytes);
      baseline.set(file, hash);
      const destination = join(configuration, file);
      mkdirSync(dirname(destination), { recursive: true });
      writeFileSync(destination, bytes, { flag: "wx", mode: 0o600 });
      if (digest(readFileSync(join(source, file))) !== hash) throw new Error("A source provider changed during its snapshot; retry with a fresh sandbox");
      let document;
      try { document = JSON.parse(bytes.toString("utf8")); }
      catch { throw new Error("A real provider document is not valid JSON; raw content is withheld"); }
      const kind = file.includes(`${sep}official${sep}`) ? "official" : file.includes(`${sep}codex${sep}`) ? "codex" : "claude";
      categories[kind] += 1;
      const declaredProtocol = document.profile?.upstream ?? document.upstreamProtocol ?? "official";
      const protocol = ["responses", "chatCompletions", "anthropicMessages", "geminiGenerateContent", "official"]
        .includes(declaredProtocol) ? declaredProtocol : "unknown";
      protocols[protocol] = (protocols[protocol] ?? 0) + 1;
    }
  } catch (error) {
    cleanupFixtureRoot(canonicalIgnoredRoot, fixtureRoot);
    throw error;
  }
  const evidence = join(canonicalSandbox, "real-provider-conversion.json");
  const manifest = join(canonicalSandbox, "source-integrity.json");
  const summary = { copiedFiles: files.length, categories, protocols, copiedOnly: "saved provider JSON", realRequests: false };
  writeFileSync(manifest, `${JSON.stringify({ ...summary, originalHashes: Object.fromEntries(baseline) }, null, 2)}\n`);
  return {
    env: { ASB_PROVIDER_CONVERSION_REAL_CONFIGURATION: configuration, ASB_PROVIDER_CONVERSION_EVIDENCE: evidence },
    summary,
    verifyOriginals() {
      const current = providerFiles(source);
      const unchanged = current.length === baseline.size && current.every((file) => baseline.get(file) === digest(readFileSync(join(source, file))));
      const verification = { ...summary, originalFilesUnchanged: unchanged };
      writeFileSync(manifest, `${JSON.stringify({ ...verification, originalHashes: Object.fromEntries(baseline) }, null, 2)}\n`);
      if (!unchanged) throw new Error("The real provider source changed during acceptance; original integrity must be investigated");
      return verification;
    },
    cleanup() { cleanupFixtureRoot(canonicalIgnoredRoot, fixtureRoot); },
  };
}

function cleanupFixtureRoot(ignoredRoot, target) {
  const canonicalTarget = realpathSync(target);
  if (!inside(ignoredRoot, canonicalTarget) || !canonicalTarget.endsWith(`${sep}real-provider-fixtures`)) {
    throw new Error("Refusing to remove an unverified provider-fixture target");
  }
  rmSync(canonicalTarget, { recursive: true, maxRetries: 10, retryDelay: 100 });
}
