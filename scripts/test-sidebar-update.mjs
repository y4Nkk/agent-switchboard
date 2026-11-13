import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";
import react from "@vitejs/plugin-react";

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outDir = path.resolve(process.argv[2] ?? path.join(repository, ".tmp", "sidebar-update"));
assert.ok(outDir.startsWith(path.join(repository, ".tmp") + path.sep), "Artifacts must stay in the workspace sandbox");
await fs.mkdir(outDir, { recursive: true });
await build({ root: repository, configFile: false, plugins: [react()], logLevel: "error",
  build: { ssr: path.join(repository, "scripts/sidebar-update.test.tsx"), outDir, emptyOutDir: false } });
const originalFetch = globalThis.fetch;
let networkRequests = 0;
globalThis.fetch = async () => { networkRequests++; throw new Error("Network access is forbidden in isolated update rendering"); };
let result;
let previewBody;
try {
  const { runChecks, previewMarkup } = await import(pathToFileURL(path.join(outDir, "sidebar-update.test.js")).href);
  result = runChecks();
  previewBody = previewMarkup();
} finally { globalThis.fetch = originalFetch; }
assert.equal(networkRequests, 0);
result = { ...result, networkRequests };
const cssBuild = await build({ root: repository, configFile: false, logLevel: "error",
  build: { write: false, rollupOptions: { input: path.join(repository, "src/styles/base.css") } } });
const outputs = (Array.isArray(cssBuild) ? cssBuild : [cssBuild]).flatMap((bundle) => bundle.output);
const css = outputs.find((entry) => entry.type === "asset" && entry.fileName.endsWith(".css"))?.source;
assert.equal(typeof css, "string", "Production CSS must compile into the isolated preview");
assert.match(css, /asb-updatebtn-ring/);
assert.match(css, /prefers-reduced-motion:reduce/);
const tokens = await fs.readFile(path.join(repository, "src/styles/tokens.css"), "utf8");
const preview = `<!doctype html><html lang="zh-CN" data-theme="light"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>侧栏更新状态预览</title>
<style>${tokens}\n${css}
body{padding:24px;background:var(--asb-content);height:auto}h1{font:var(--asb-text-title)}
h2{font:var(--asb-text-section);margin-top:32px}.preview-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:20px}
.preview-case{padding:16px;border:1px solid var(--asb-hairline);border-radius:var(--asb-radius)}
.preview-case h3{font:var(--asb-text-caption);color:var(--asb-text-muted);margin:0 0 12px}
.preview-rails{display:flex;gap:12px;align-items:start}.preview-rail{display:block;width:var(--asb-sidebar-width);height:auto;flex:none;border-radius:var(--asb-radius)}
.preview-rail ul{padding-block:4px}.preview-note{color:var(--asb-text-muted);font:var(--asb-text-label)}
</style><body><h1>侧栏更新状态</h1><p class="preview-note">隔离状态示例 · 静态渲染，不连接更新服务。</p>
<button class="asb-btn asb-btn-secondary" onclick="document.documentElement.dataset.theme=document.documentElement.dataset.theme==='dark'?'light':'dark'">切换明暗主题</button>
${previewBody}</body></html>`;
await fs.writeFile(path.join(outDir, "preview.html"), preview);
await fs.writeFile(path.join(outDir, "render-evidence.json"), `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify({ ...result, cssCompiled: true, preview: path.join(outDir, "preview.html") }));
