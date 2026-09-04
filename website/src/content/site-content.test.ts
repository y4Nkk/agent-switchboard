import { describe, expect, it } from "vitest";

import { configurationAssembly } from "../generated/configuration-assembly";
import { siteContent, siteContentByLocale } from "./site-content";

describe("site-content 契约", () => {
  it("下载只指向 GitHub Releases 最新页", () => {
    expect(siteContent.releasesUrl).toBe(
      "https://github.com/y4Nkk/agent-switchboard/releases/latest",
    );
    for (const action of [...siteContent.hero.actions, ...siteContent.final.actions]) {
      if (action.label === "下载版本") {
        expect(action.href).toBe(siteContent.releasesUrl);
      }
    }
  });

  it("全站不出现版本号或安装包文件名", () => {
    const serialized = JSON.stringify(siteContent);
    expect(serialized).not.toMatch(/v?\d+\.\d+\.\d+/);
    expect(serialized).not.toMatch(/\.(exe|dmg|AppImage|deb|tar\.gz)/);
  });

  it("站点不使用产品截图，视觉证据全部来自 DOM 复刻", () => {
    const serialized = JSON.stringify(siteContent);
    expect(serialized).not.toMatch(/\.png/);
    expect(serialized).not.toMatch(/\.jpe?g/);
  });

  it("路由卡展示新模型与 Amazon Bedrock 路线", () => {
    const models = siteContent.hero.cards.map((card) => card.model);
    expect(models).toEqual(["GPT-6 Astra", "Claude Opus 5.1"]);
    for (const card of siteContent.hero.cards) {
      expect(card.endpoint).toBe("bedrock-runtime.us-east-1.amazonaws.com");
      expect(card.provider).toBe("Amazon Bedrock");
    }
  });

  it("导航锚点指向页内区块", () => {
    for (const item of siteContent.nav) {
      if (!item.external) {
        expect(item.href.startsWith("#")).toBe(true);
      }
    }
  });

  it("配置积木台如实区分 Codex TOML 与 Claude Code JSON 输出", () => {
    expect(configurationAssembly.codex.fileName).toBe("config.toml");
    expect(configurationAssembly.codex.codeLines).toContain('model_provider = "openai"');
    expect(configurationAssembly.claude.fileName).toBe("settings.json");
    expect(configurationAssembly.claude.codeLines).toContain('  "autoCompactEnabled": true,');
    expect(configurationAssembly.claude.codeLines).toContain('    "ANTHROPIC_AUTH_TOKEN": "••••••••"');
  });

  it("中英文内容保持同一导航与积木客户端集合", () => {
    const english = siteContentByLocale.en;
    expect(english.nav.map((item) => item.href)).toEqual(siteContent.nav.map((item) => item.href));
    expect(Object.keys(english.assembly.clients)).toEqual(Object.keys(siteContent.assembly.clients));
    expect(english.hero.actions.map((item) => item.href)).toEqual(
      siteContent.hero.actions.map((item) => item.href),
    );
  });
});
