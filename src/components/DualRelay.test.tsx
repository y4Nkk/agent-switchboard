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
  it("shows both client cards with text state, not color alone", () => {
    render(
      <DualRelay
        routes={{ codex: route("codex", "中继 A", "gpt-5.1"), claude: null }}
        providerNames={{ codex: "中继 A", claude: "未加载" }}
        selectedProfile={null}
        canSwitch={false}
        busy={false}
        onPreview={() => {}}
        onSwitch={() => {}}
      />,
    );

    expect(screen.getByText("Codex")).toBeInTheDocument();
    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "当前路由" })).toBeInTheDocument();
    expect(screen.getByRole("article", { name: "Codex 路由" })).toHaveTextContent("中继 A");
    expect(screen.getByText("gpt-5.1")).toBeInTheDocument();
    expect(screen.getByText("relay.internal")).toBeInTheDocument();
    expect(screen.getAllByText("未加载").length).toBeGreaterThan(0);
    expect(screen.getAllByText("已启用").length).toBe(1);
    expect(document.querySelectorAll(".asb-route-card.is-on .asb-card-particle-canvas").length).toBe(1);
    expect(document.querySelectorAll(".asb-route-card:not(.is-on) .asb-card-particle-canvas").length).toBe(1);
  });

  it("shows the matched profile name instead of the connection mode", () => {
    render(
      <DualRelay
        routes={{
          codex: route("codex", null, null, "custom"),
          claude: route("claude", null, null, "official"),
        }}
        providerNames={{ codex: "智谱 GLM", claude: "官方登录" }}
        selectedProfile={null}
        canSwitch={false}
        busy={false}
        onPreview={() => {}}
        onSwitch={() => {}}
      />,
    );
    expect(screen.getByText("智谱 GLM")).toBeInTheDocument();
    expect(screen.getAllByText("官方登录").length).toBeGreaterThan(0);
  });

  it("keeps switch actions disabled until a profile is selected", () => {
    render(
      <DualRelay
        routes={{ codex: route("codex", "中继 A", "gpt-5.1"), claude: null }}
        providerNames={{ codex: "中继 A", claude: "未加载" }}
        selectedProfile={null}
        canSwitch={false}
        busy={false}
        onPreview={() => {}}
        onSwitch={() => {}}
      />,
    );
    expect(screen.getByRole("button", { name: "查看变更" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "安全切换" })).toBeDisabled();
  });
});
