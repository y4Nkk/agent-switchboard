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
        nsis: {
          headerImage: string;
          sidebarImage: string;
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
      headerImage: "windows/installer-header.bmp",
      sidebarImage: "windows/installer-sidebar.bmp",
      installerIcon: "icons/icon.ico",
      uninstallerIcon: "icons/icon.ico",
      languages: ["SimpChinese", "English"],
    });

    expect(readBundle("tauri.macos.conf.json").targets).toEqual(["dmg", "app"]);
    expect(readBundle("tauri.linux.conf.json").targets).toEqual([
      "deb",
      "appimage",
    ]);

    for (const [asset, width, height] of [
      [windows.windows.nsis.headerImage, 150, 57],
      [windows.windows.nsis.sidebarImage, 164, 314],
    ] as const) {
      const bitmap = readFileSync(join(repoRoot, "src-tauri", asset));
      expect(bitmap.subarray(0, 2).toString("ascii")).toBe("BM");
      expect(bitmap.readUInt32LE(18)).toBe(width);
      expect(bitmap.readUInt32LE(22)).toBe(height);
    }
  });

  it("publishes a complete signed updater release only after all payloads are ready", () => {
    const workflow = readFileSync(
      join(repoRoot, ".github", "workflows", "package.yml"),
      "utf8",
    );

    expect(workflow).toContain(
      "if: ${{ !cancelled() && startsWith(github.ref, 'refs/tags/v') && needs.build.result == 'success' }}",
    );
    expect(workflow).toContain("TAURI_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("src-tauri/tauri.updater.conf.json");
    expect(workflow).toContain("pattern: agent-switchboard-*");
    expect(workflow).toContain("node scripts/updater-release.mjs manifest");
    expect(workflow).toContain(
      'gh release upload "$GITHUB_REF_NAME" "${assets[@]}" --clobber',
    );
    expect(workflow).toContain('[[ "$asset" == "dist/latest.json" ]] && continue');
    expect(workflow).toContain('gh release upload "$GITHUB_REF_NAME" dist/latest.json --clobber');
    expect(workflow.indexOf('gh release upload "$GITHUB_REF_NAME" "${assets[@]}" --clobber')).toBeLessThan(
      workflow.indexOf('gh release upload "$GITHUB_REF_NAME" dist/latest.json --clobber'),
    );
    expect(workflow).not.toContain("always() && !cancelled()");
  });
});
