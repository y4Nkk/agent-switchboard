import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { loadConfigFromFile } from "vite";

const srcRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = dirname(srcRoot);

function collectSourceFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === "test") continue;
      files.push(...collectSourceFiles(full));
    } else if (/\.(ts|tsx)$/.test(entry) && !/\.test\./.test(entry)) {
      files.push(full);
    }
  }
  return files;
}

describe("UI boundary", () => {
  it("lets only the api client talk to the backend", () => {
    const offenders: string[] = [];
    for (const file of collectSourceFiles(srcRoot)) {
      const rel = relative(srcRoot, file).replace(/\\/g, "/");
      const text = readFileSync(file, "utf8");
      if (rel !== "api/client.ts" && text.includes("@tauri-apps/api")) {
        offenders.push(`${rel} imports the backend directly`);
      }
      if (rel !== "api/client.ts" && /from ["']node:(fs|path)["']/.test(text)) {
        offenders.push(`${rel} uses node fs/path directly`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it("keeps components from constructing configuration file text", () => {
    const offenders: string[] = [];
    for (const file of collectSourceFiles(join(srcRoot, "components"))) {
      const text = readFileSync(file, "utf8");
      if (/config\.toml|settings\.json/.test(text)) {
        offenders.push(relative(srcRoot, file));
      }
    }
    expect(offenders).toEqual([]);
  });

  it("derives the Vite listener from the Tauri development URL", async () => {
    const tauriConfig = JSON.parse(
      readFileSync(join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8"),
    ) as { build: { devUrl: string } };
    const configuredUrl = new URL(tauriConfig.build.devUrl);
    const loadedViteConfig = await loadConfigFromFile(
      { command: "serve", mode: "test" },
      join(repoRoot, "vite.config.ts"),
    );

    expect(configuredUrl.origin).toBe("http://127.0.0.1:1420");
    expect(loadedViteConfig?.config.server?.host).toBe(configuredUrl.hostname);
    expect(loadedViteConfig?.config.server?.port).toBe(Number(configuredUrl.port));
  });

  it("keeps the platform bundle contracts in their own Tauri config files", () => {
    const readBundle = (file: string) =>
      (
        JSON.parse(
          readFileSync(join(repoRoot, "src-tauri", file), "utf8"),
        ) as { bundle: Record<string, unknown> }
      ).bundle;

    const base = readBundle("tauri.conf.json");
    expect(base.icon).toEqual([
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico",
    ]);
    // The platform-neutral base never bundles anything and carries no
    // Windows-only resources; per-OS overlay files own those contracts.
    expect(base.targets).toBeUndefined();
    expect(base.resources).toBeUndefined();
    expect(base.windows).toBeUndefined();

    const windows = readBundle("tauri.windows.conf.json") as {
      targets: string[];
      resources: Record<string, string>;
      windows: {
        webviewInstallMode: { type: string };
        nsis: {
          installerIcon: string;
          uninstallerIcon: string;
          languages: string[];
        };
      };
    };
    expect(windows.targets).toEqual(["nsis"]);
    expect(windows.resources).toEqual({
      "bin/WebView2Loader.dll": "WebView2Loader.dll",
    });
    expect(windows.windows.nsis).toEqual({
      installerIcon: "icons/icon.ico",
      uninstallerIcon: "icons/icon.ico",
      languages: ["SimpChinese", "English"],
    });
    expect(windows.windows.webviewInstallMode).toEqual({ type: "skip" });

    expect(readBundle("tauri.macos.conf.json").targets).toEqual(["dmg", "app"]);
    expect(readBundle("tauri.linux.conf.json").targets).toEqual([
      "deb",
      "appimage",
    ]);
  });

  it("uses one custom Windows installer for manual installation and updates", () => {
    const packageConfig = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
    const builder = readFileSync(join(repoRoot, "scripts", "build-windows-installer.ps1"), "utf8");
    const release = readFileSync(join(repoRoot, "scripts", "updater-release.mjs"), "utf8");
    expect(packageConfig.scripts["tauri:build:windows"]).toContain("scripts/build-windows-installer.ps1");
    expect(builder).toContain("node node_modules/@tauri-apps/cli/tauri.js build --config src-tauri/tauri.windows.conf.json @TauriArguments");
    expect(builder).toContain("AgentSwitchboard.Installer.Engine.exe");
    expect(builder).toContain("AgentSwitchboard.Installer.Theme.xaml");
    expect(builder).toContain("node node_modules/@tauri-apps/cli/tauri.js signer sign $output");
    expect(builder).toContain("$engineVersion -ne $version");
    expect(builder).not.toContain("$EnginePath");
    expect(release).toContain('directory: "installer"');
    expect(release).not.toContain('directory: "nsis"');
  });

  it("lets the native tray window own its only outer outline", () => {
    const config = JSON.parse(readFileSync(join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8"));
    const tray = config.app.windows.find((window: { label: string }) => window.label === "tray");
    expect(tray.decorations).toBe(false);
    expect(tray.shadow).toBe(false);
    expect(tray.transparent).toBe(true);
    const css = readFileSync(join(repoRoot, "src", "tray", "tray.css"), "utf8");
    const panel = css.match(/\.tray-panel\s*\{([^}]+)\}/)?.[1];
    expect(panel).toContain("border: 0;");
    expect(panel).toContain("border-radius: 0;");
    expect(css).not.toContain("--asb-tray-radius");
    const popup = readFileSync(join(repoRoot, "src-tauri", "src", "tray", "popup.rs"), "utf8");
    expect(popup).toContain("DWMWA_COLOR_NONE");
  });

  it("publishes the complete direct-update release only after all payloads are ready", () => {
    const workflow = readFileSync(
      join(repoRoot, ".github", "workflows", "package.yml"),
      "utf8",
    );

    expect(workflow).toContain(
      "if: ${{ !cancelled() && startsWith(github.ref, 'refs/tags/v') && needs.build.result == 'success' }}",
    );
    expect(workflow).toContain("TAURI_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("src-tauri/tauri.updater.conf.json");
    expect(workflow).toContain("verify_updater_artifact");
    expect(workflow).toContain("pattern: agent-switchboard-release-*");
    expect(workflow).toContain("node scripts/updater-release.mjs manifest");
    expect(workflow).toContain(
      'gh release upload "$GITHUB_REF_NAME" "${assets[@]}" --clobber',
    );
    expect(workflow).toContain('[[ "$asset" == "dist/latest.json" ]] && continue');
    expect(workflow).toContain('gh release upload "$GITHUB_REF_NAME" dist/latest.json --clobber');
    expect(workflow.indexOf('gh release upload "$GITHUB_REF_NAME" "${assets[@]}" --clobber')).toBeLessThan(
      workflow.indexOf('gh release upload "$GITHUB_REF_NAME" dist/latest.json --clobber'),
    );
    expect(workflow).toContain("npm run msix:package");
    expect(workflow).toContain("agent-switchboard-store-windows-x64");
    expect(workflow).toContain("target/release/bundle/msix/*.msix");
    expect(workflow).not.toContain("always() && !cancelled()");
  });

  it("keeps the Microsoft Store identity and supported package validation in the MSIX contract", () => {
    const identity = JSON.parse(
      readFileSync(join(repoRoot, "src-tauri", "windows", "store-identity.json"), "utf8"),
    );
    const packageScript = readFileSync(join(repoRoot, "scripts", "package-msix.ps1"), "utf8");

    expect(identity).toEqual({
      name: "yy1.AgentSwitchboard",
      publisher: "CN=FFA766F3-961D-4013-91DD-95D1867EBC0F",
      publisherDisplayName: "yy1",
      displayName: "Agent Switchboard",
      description: "Manage Codex and Claude Code configuration.",
    });
    expect(packageScript).toContain("MakeAppx.exe");
    expect(packageScript).toContain("& $makeAppx pack /d $stageRoot /p $outputPath /o");
    expect(packageScript).toContain("& $makeAppx unpack /p $outputPath /d $validationRoot /o");
    expect(packageScript).toContain("$requiredPackageFiles");
    expect(packageScript).not.toContain("makeAppx validate");
  });

  it("builds the canonical Linux installer assets in CNB", () => {
    const pipeline = readFileSync(join(repoRoot, ".cnb.yml"), "utf8");

    expect(pipeline).toMatch(/^\$:\r?\n  push:/m);
    expect(pipeline).toContain("libxdo-dev");
    expect(pipeline).toContain("powershell");
    expect(pipeline).toContain("RUSTUP_TOOLCHAIN=stable");
    expect(pipeline).toContain("npm run test:updater-release");
    expect(pipeline).toContain("npm run tauri:build:linux");
    expect(pipeline).toContain("--build linux-x64");
    expect(pipeline).toContain("--bundle-directory target/release/bundle");
    expect(pipeline).toContain("--output-directory release-assets");
    expect(pipeline).toContain("assert-release-secret-free.ps1");
    expect(pipeline).toContain("image: cnbcool/attachments:latest");
    expect(pipeline).toContain('"./release-assets/*"');
  });
});
