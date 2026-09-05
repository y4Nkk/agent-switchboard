import { cp, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE_PREFIX = "agent-switchboard";

const BUILD_ARTIFACTS = {
  "windows-x64": [
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_windows-x86_64-nsis.exe`,
      directory: "installer",
      matcher: /-setup\.exe$/i,
      signed: true,
    },
  ],
  "macos-arm64": [
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_darwin-aarch64.dmg`,
      directory: "dmg",
      matcher: /\.dmg$/i,
      signed: false,
    },
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_darwin-aarch64.app.tar.gz`,
      directory: "macos",
      matcher: /\.app\.tar\.gz$/i,
      signed: true,
      updaterOnly: true,
    },
  ],
  "macos-x64": [
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_darwin-x86_64.dmg`,
      directory: "dmg",
      matcher: /\.dmg$/i,
      signed: false,
    },
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_darwin-x86_64.app.tar.gz`,
      directory: "macos",
      matcher: /\.app\.tar\.gz$/i,
      signed: true,
      updaterOnly: true,
    },
  ],
  "linux-x64": [
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_linux-x86_64.deb`,
      directory: "deb",
      matcher: /\.deb$/i,
      signed: true,
    },
    {
      destination: (version) => `${PACKAGE_PREFIX}_${version}_linux-x86_64.AppImage`,
      directory: "appimage",
      matcher: /\.AppImage$/i,
      signed: true,
    },
  ],
};

const PLATFORM_ASSETS = [
  ["windows-x86_64-nsis", (version) => `${PACKAGE_PREFIX}_${version}_windows-x86_64-nsis.exe`],
  ["darwin-aarch64-app", (version) => `${PACKAGE_PREFIX}_${version}_darwin-aarch64.app.tar.gz`],
  ["darwin-x86_64-app", (version) => `${PACKAGE_PREFIX}_${version}_darwin-x86_64.app.tar.gz`],
  ["linux-x86_64-deb", (version) => `${PACKAGE_PREFIX}_${version}_linux-x86_64.deb`],
  ["linux-x86_64-appimage", (version) => `${PACKAGE_PREFIX}_${version}_linux-x86_64.AppImage`],
];

function assertVersion(version) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid release version: ${version}`);
  }
}

function assertRepository(repository) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error(`Invalid GitHub repository: ${repository}`);
  }
}

async function ensureEmptyDirectory(directory) {
  await mkdir(directory, { recursive: true });
  const entries = await readdir(directory);
  if (entries.length > 0) {
    throw new Error(`Release asset output directory must be empty: ${directory}`);
  }
}

async function findOnlyBundleAsset(bundleDirectory, definition) {
  const directory = path.join(bundleDirectory, definition.directory);
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (error.code === "ENOENT") {
      throw new Error(`Missing bundle directory: ${directory}`);
    }
    throw error;
  }

  const matches = entries.filter((entry) => entry.isFile() && definition.matcher.test(entry.name));
  if (matches.length !== 1) {
    throw new Error(
      `Expected exactly one updater bundle in ${directory}, found ${matches.length}`,
    );
  }
  return path.join(directory, matches[0].name);
}

export async function stageBuildArtifacts({
  build,
  bundleDirectory,
  includeUpdater,
  outputDirectory,
  version,
}) {
  assertVersion(version);
  const definitions = BUILD_ARTIFACTS[build];
  if (!definitions) {
    throw new Error(`Unsupported build target: ${build}`);
  }

  await ensureEmptyDirectory(outputDirectory);
  for (const definition of definitions) {
    if (definition.updaterOnly && !includeUpdater) {
      continue;
    }

    const source = await findOnlyBundleAsset(bundleDirectory, definition);
    const destination = path.join(outputDirectory, definition.destination(version));
    await cp(source, destination);

    if (includeUpdater && definition.signed) {
      const signature = `${source}.sig`;
      try {
        const signatureInfo = await stat(signature);
        if (!signatureInfo.isFile()) {
          throw new Error("not a file");
        }
      } catch {
        throw new Error(`Missing updater signature: ${signature}`);
      }
      await cp(signature, `${destination}.sig`);
    }
  }
}

export async function readWorkspaceVersion(cargoManifest) {
  const contents = await readFile(cargoManifest, "utf8");
  const header = contents.match(/^\[workspace\.package\]\s*$/m);
  if (!header || header.index === undefined) {
    throw new Error(`Missing [workspace.package] in ${cargoManifest}`);
  }
  const afterHeader = contents.slice(header.index + header[0].length);
  const nextSection = afterHeader.search(/^\s*\[/m);
  const section = nextSection === -1 ? afterHeader : afterHeader.slice(0, nextSection);
  const version = section.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) {
    throw new Error(`Missing workspace package version in ${cargoManifest}`);
  }
  assertVersion(version);
  return version;
}

export async function assertReleaseVersion({ cargoManifest, tag, version }) {
  assertVersion(version);
  const workspaceVersion = await readWorkspaceVersion(cargoManifest);
  if (workspaceVersion !== version) {
    throw new Error(`Release version ${version} does not match Cargo.toml version ${workspaceVersion}`);
  }
  if (tag !== `v${version}`) {
    throw new Error(`Release tag ${tag} must equal v${version}`);
  }
}

async function requireAsset(inputDirectory, assetName) {
  const assetPath = path.join(inputDirectory, assetName);
  try {
    const assetInfo = await stat(assetPath);
    if (!assetInfo.isFile()) {
      throw new Error("not a file");
    }
  } catch {
    throw new Error(`Missing release asset: ${assetName}`);
  }
  return assetPath;
}

export async function generateUpdaterManifest({
  cargoManifest,
  inputDirectory,
  outputFile,
  pubDate,
  repository,
  tag,
  version,
}) {
  assertRepository(repository);
  await assertReleaseVersion({ cargoManifest, tag, version });
  const platforms = {};

  for (const [platform, assetForVersion] of PLATFORM_ASSETS) {
    const assetName = assetForVersion(version);
    await requireAsset(inputDirectory, assetName);
    const signaturePath = await requireAsset(inputDirectory, `${assetName}.sig`);
    const signature = await readFile(signaturePath, "utf8");
    if (signature.trim().length === 0) {
      throw new Error(`Empty updater signature: ${assetName}.sig`);
    }
    platforms[platform] = {
      signature,
      url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(assetName)}`,
    };
  }

  const manifest = {
    version,
    notes: "",
    pub_date: pubDate ?? new Date().toISOString(),
    platforms,
  };
  await writeFile(outputFile, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  return manifest;
}

function parseArguments(argumentsList) {
  const [command, ...rest] = argumentsList;
  const options = {};
  for (let index = 0; index < rest.length; index += 1) {
    const argument = rest[index];
    if (argument === "--include-updater") {
      options.includeUpdater = true;
      continue;
    }
    if (!argument.startsWith("--") || index + 1 === rest.length) {
      throw new Error(`Invalid argument: ${argument}`);
    }
    const optionName = argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    options[optionName] = rest[index + 1];
    index += 1;
  }
  return { command, options };
}

async function main() {
  const { command, options } = parseArguments(process.argv.slice(2));
  if (command === "stage") {
    await stageBuildArtifacts({
      build: options.build,
      bundleDirectory: options.bundleDirectory,
      includeUpdater: options.includeUpdater ?? false,
      outputDirectory: options.outputDirectory,
      version: options.version,
    });
    return;
  }
  if (command === "validate-version") {
    await assertReleaseVersion({
      cargoManifest: options.cargoManifest,
      tag: options.tag,
      version: options.version,
    });
    return;
  }
  if (command === "workspace-version") {
    console.log(await readWorkspaceVersion(options.cargoManifest));
    return;
  }
  if (command === "manifest") {
    await generateUpdaterManifest({
      cargoManifest: options.cargoManifest,
      inputDirectory: options.inputDirectory,
      outputFile: options.outputFile,
      pubDate: options.pubDate,
      repository: options.repository,
      tag: options.tag,
      version: options.version,
    });
    return;
  }
  throw new Error(
    `Expected command stage, validate-version, workspace-version, or manifest; received ${command ?? "nothing"}`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
