import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import App from "./App";
import { siteContent, siteContentByLocale } from "./content/site-content";

describe("App", () => {
  it("渲染首屏标题与事实句", () => {
    render(<App />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(
      siteContent.hero.title,
    );
    expect(screen.getByText(siteContent.hero.fact)).toBeTruthy();
  });

  it("下载入口只保留首屏一处，并指向 Releases 最新页", () => {
    render(<App />);
    const links = screen.getAllByRole("link", { name: "下载版本" });
    expect(links.length).toBe(1);
    for (const link of links) {
      expect(link.getAttribute("href")).toBe(siteContent.releasesUrl);
    }
  });

  it("收尾提供查看源码与点亮 Star 两个入口，均指向仓库", () => {
    render(<App />);
    for (const label of ["查看源码", "点亮 Star"]) {
      for (const link of screen.getAllByRole("link", { name: label })) {
        expect(link.getAttribute("href")).toBe(siteContent.repoUrl);
      }
    }
  });

  it("路由卡复刻展示两个客户端的新模型", () => {
    render(<App />);
    for (const card of siteContent.hero.cards) {
      expect(screen.getAllByText(card.model).length).toBeGreaterThan(0);
      expect(screen.getAllByText(card.provider).length).toBe(2);
    }
  });

  it("预览复刻展示脱敏差异与新模型", () => {
    render(<App />);
    expect(screen.getAllByText(siteContent.preview.title).length).toBe(2);
    expect(screen.getAllByText("GPT-6 Astra").length).toBeGreaterThan(1);
    expect(screen.getAllByText("••••••••").length).toBe(2);
  });

  it("配置积木台默认组合 Codex 的通用与供应商设置到 TOML", () => {
    render(<App />);
    expect(screen.getByText("Codex 通用配置")).toBeTruthy();
    expect(screen.getByText("Amazon Bedrock 设置")).toBeTruthy();
    expect(screen.getByText("config.toml")).toBeTruthy();
    expect(screen.getAllByText('model_provider = "openai"').length).toBeGreaterThan(0);
    expect(document.querySelector(".assembly-control-segments")).toBeTruthy();
    expect(document.querySelector(".assembly-control-slider")).toBeTruthy();
  });

  it("配置积木台切换 Claude Code 后展示 JSON 目标文件", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("tab", { name: "Claude Code" }));
    expect(screen.getByText("Claude Code 通用配置")).toBeTruthy();
    expect(screen.getByText("settings.json")).toBeTruthy();
    expect(screen.getByText('"autoCompactEnabled": true,')).toBeTruthy();
  });

  it("配置积木台遵循键盘标签页操作", () => {
    render(<App />);
    const codex = screen.getByRole("tab", { name: "Codex" });
    const claude = screen.getByRole("tab", { name: "Claude Code" });

    codex.focus();
    fireEvent.keyDown(codex, { key: "ArrowRight" });

    expect(claude.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(claude);
    fireEvent.keyDown(claude, { key: "Home" });
    expect(document.activeElement).toBe(codex);
  });

  it("语言菜单切换整页文案到 English", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: siteContent.header.localeMenuLabel }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: /English$/ }));
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(
      siteContentByLocale.en.hero.title,
    );
    expect(screen.getAllByText("Model").length).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "Download" })).toBeTruthy();
  });

  it("语言菜单支持方向键选择与 Escape 返回触发器", () => {
    render(<App />);
    const trigger = screen.getByRole("button", { name: siteContent.header.localeMenuLabel });
    fireEvent.click(trigger);
    const chinese = screen.getByRole("menuitemradio", { name: /中文$/ });
    const english = screen.getByRole("menuitemradio", { name: /English$/ });

    chinese.focus();
    fireEvent.keyDown(chinese, { key: "ArrowDown" });
    expect(document.activeElement).toBe(english);
    fireEvent.keyDown(english, { key: "Escape" });
    expect(document.activeElement).toBe(trigger);
  });

  it("顶栏以语言、主题和 GitHub 控制组取代下载按钮", () => {
    render(<App />);
    expect(screen.getByRole("button", { name: siteContent.header.localeMenuLabel })).toBeTruthy();
    expect(screen.getByRole("button", { name: siteContent.header.themeToDarkLabel })).toBeTruthy();
    expect(
      screen.getByRole("link", { name: siteContent.header.githubLabel }).getAttribute("href"),
    ).toBe(siteContent.repoUrl);
  });

  it("主题控制会切换根文档外观状态", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: siteContent.header.themeToDarkLabel }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(screen.getByRole("button", { name: siteContent.header.themeToLightLabel })).toBeTruthy();
  });
});
