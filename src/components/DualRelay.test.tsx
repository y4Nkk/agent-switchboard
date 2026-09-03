import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { DualRelay } from "./DualRelay";
import type { RouteState } from "../api/client";

function route(
  app: "codex" | "claude",
  name: string | null,
  model: string | null,
  mode: "official" | "custom" = "custom",
): RouteState {
  return {
    app,
    routeMode: mode,
    providerName: name,
    model,
    baseUrl: mode === "custom" ? "https://relay.internal" : null,
    apiKey: "test-api-key",
    wireApi: app === "codex" ? "responses" : null,
    codexModelOptions: null,
    haikuModel: null,
    sonnetModel: null,
    opusModel: null,
    availableModels: null,
    scopeWarnings: [],
  };
}

describe("DualRelay", () => {
  it("shows both client cards with route facts and unloaded state text", () => {
    render(
      <DualRelay
        routes={{ codex: route("codex", "中继 A", "gpt-5.1"), claude: null }}
        providerNames={{ codex: "中继 A", claude: "未加载" }}
      />,
    );

    expect(screen.getByText("Codex")).toBeInTheDocument();
    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "当前启用配置" })).toBeInTheDocument();
    expect(screen.getByRole("article", { name: "Codex 当前配置" })).toHaveTextContent("中继 A");
    expect(screen.getByText("gpt-5.1")).toBeInTheDocument();
    expect(screen.getByText("relay.internal")).toBeInTheDocument();
    expect(screen.getAllByText("未加载").length).toBeGreaterThan(0);
    expect(screen.queryByText("已启用")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".asb-route-card.is-on .asb-card-particle-canvas").length).toBe(1);
    expect(document.querySelector(".asb-route-card.is-on .asb-starlight")).toHaveAttribute(
      "data-active",
      "true",
    );
    expect(document.querySelector(".asb-route-card.is-on .asb-starlight")).toHaveAttribute(
      "data-variant",
      "cool",
    );
    expect(document.querySelector(".asb-route-card:not(.is-on) .asb-starlight")).toHaveAttribute(
      "data-active",
      "false",
    );
  });

  it("renders no action buttons; switching lives on the provider page", () => {
    render(
      <DualRelay
        routes={{
          codex: route("codex", null, null, "custom"),
          claude: route("claude", null, null, "official"),
        }}
        providerNames={{ codex: "智谱 GLM", claude: "官方登录" }}
      />,
    );
    expect(screen.getByText("智谱 GLM")).toBeInTheDocument();
    expect(screen.getAllByText("官方登录").length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "查看变更" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "安全切换" })).not.toBeInTheDocument();
    expect(document.querySelector('.asb-route-card[data-app="codex"] .asb-starlight')).toHaveAttribute(
      "data-variant",
      "cool",
    );
    expect(document.querySelector('.asb-route-card[data-app="claude"] .asb-starlight')).toHaveAttribute(
      "data-variant",
      "violet",
    );
  });
});
