import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE_PREFIX = "agent-switchboard";

function assertStableReleaseVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(
      "Microsoft Store packages require a stable major.minor.patch version; received " +
        version,
    );
  }
  const components = match.slice(1).map(Number);
  if (
    components.some(
      (component) => !Number.isSafeInteger(component) || component > 65535,
    )
  ) {
    throw new Error(
      "Microsoft Store version components must be integers from 0 to 65535; received " +
        version,
    );
  }
  return components;
}

function escapeXml(value) {
  return value.replace(/[&<>"']/g, (character) => {
    const entities = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&apos;",
    };
    return entities[character];
  });
}

function assertIdentity(identity) {
  for (const field of [
    "name",
    "publisher",
    "publisherDisplayName",
    "displayName",
    "description",
  ]) {
    if (
      typeof identity[field] !== "string" ||
      identity[field].trim().length === 0
    ) {
      throw new Error("Microsoft Store identity is missing " + field);
    }
  }
  return identity;
}

/**
 * MSIX requires four numeric components and reserves the fourth for the Store.
 * Cargo's major is offset by one because a Store package cannot start at zero.
 */
export function toMsixVersion(releaseVersion) {
  const [major, minor, patch] = assertStableReleaseVersion(releaseVersion);
  if (major === 65535) {
    throw new Error(
      "Microsoft Store version cannot map a Cargo major of 65535: " +
        releaseVersion,
    );
  }
  return [major + 1, minor, patch, 0].join(".");
}

export function msixAssetName(releaseVersion) {
  assertStableReleaseVersion(releaseVersion);
  return PACKAGE_PREFIX + "_" + releaseVersion + "_windows-x86_64.msix";
}

export function createMsixManifest(identity, releaseVersion) {
  const storeIdentity = assertIdentity(identity);
  const version = toMsixVersion(releaseVersion);
  return [
    '<?xml version="1.0" encoding="utf-8"?>',
    "<Package",
    '  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"',
    '  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"',
    '  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"',
    '  IgnorableNamespaces="uap rescap">',
    "  <Identity",
    '    Name="' + escapeXml(storeIdentity.name) + '"',
    '    Publisher="' + escapeXml(storeIdentity.publisher) + '"',
    '    Version="' + version + '"',
    '    ProcessorArchitecture="x64" />',
    "  <Properties>",
    "    <DisplayName>" +
      escapeXml(storeIdentity.displayName) +
      "</DisplayName>",
    "    <PublisherDisplayName>" +
      escapeXml(storeIdentity.publisherDisplayName) +
      "</PublisherDisplayName>",
    "    <Logo>Assets\\StoreLogo.png</Logo>",
    "  </Properties>",
    "  <Resources>",
    '    <Resource Language="en-us" />',
    "  </Resources>",
    "  <Dependencies>",
    '    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />',
    "  </Dependencies>",
    "  <Capabilities>",
    '    <rescap:Capability Name="runFullTrust" />',
    "  </Capabilities>",
    "  <Applications>",
    '    <Application Id="AgentSwitchboard" Executable="agent-switchboard.exe" EntryPoint="Windows.FullTrustApplication">',
    "      <uap:VisualElements",
    '        DisplayName="' + escapeXml(storeIdentity.displayName) + '"',
    '        Description="' + escapeXml(storeIdentity.description) + '"',
    '        BackgroundColor="transparent"',
    '        Square150x150Logo="Assets\\Square150x150Logo.png"',
    '        Square44x44Logo="Assets\\Square44x44Logo.png" />',
    "    </Application>",
    "  </Applications>",
    "</Package>",
    "",
  ].join("\n");
}

async function readIdentity(identityPath) {
  let identity;
  try {
    identity = JSON.parse(await readFile(identityPath, "utf8"));
  } catch (error) {
    throw new Error(
      "Could not read Microsoft Store identity " +
        identityPath +
        ": " +
        error.message,
    );
  }
  return assertIdentity(identity);
}

function parseArguments(argumentsList) {
  const [command, ...rest] = argumentsList;
  const options = {};
  for (let index = 0; index < rest.length; index += 2) {
    const option = rest[index];
    const value = rest[index + 1];
    if (!option?.startsWith("--") || value === undefined) {
      throw new Error("Invalid argument: " + (option ?? "nothing"));
    }
    options[
      option.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
    ] = value;
  }
  return { command, options };
}

async function main() {
  const { command, options } = parseArguments(process.argv.slice(2));
  if (command === "package-name") {
    console.log(msixAssetName(options.version));
    return;
  }
  if (command === "manifest") {
    if (!options.identity || !options.output || !options.version) {
      throw new Error("manifest requires --identity, --output, and --version");
    }
    const manifest = createMsixManifest(
      await readIdentity(options.identity),
      options.version,
    );
    await writeFile(options.output, manifest, "utf8");
    return;
  }
  throw new Error(
    "Expected command package-name or manifest; received " +
      (command ?? "nothing"),
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
