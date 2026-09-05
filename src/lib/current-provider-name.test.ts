import { describe, expect, it } from "vitest";
import type { ConfigFileStatus, ProviderProfile } from "../api/client";
import { currentProviderName } from "./current-provider-name";

function status(matchStatus: ConfigFileStatus["matchStatus"]): ConfigFileStatus {
  return {
    app: "claude",
    path: "C:/Users/test/.claude/settings.json",
    exists: true,
    syntaxOk: true,
    route: {
      app: "claude",
      routeMode: "custom",
      providerName: null,
      model: "glm-5.3-flash[1m]",
      baseUrl: "https://open.bigmodel.cn",
      apiKey: "test-api-key",
      wireApi: null,
      codexModelOptions: null,
      haikuModel: null,
      sonnetModel: null,
      opusModel: null,
      availableModels: null,
      scopeWarnings: [],
    },
    readError: null,
    activeProfileId: null,
    matchStatus,
    lastSwitch: null,
  };
}

describe("currentProviderName", () => {
  const profiles: ProviderProfile[] = [{
    id: "zhipu", app: "claude", name: "Zhipu GLM", routeMode: "custom",
    model: null, baseUrl: "https://open.bigmodel.cn", apiKey: "test-api-key",
    modelOptions: null, websiteUrl: null,
  }];
  it("uses the active matching profile name over the custom connection mode", () => {
    expect(
      currentProviderName(
        { ...status({ kind: "matchesProfile", profileId: "zhipu", profileName: "Old name" }), activeProfileId: "zhipu" },
        profiles,
      ),
    ).toBe("Zhipu GLM");
  });

  it("does not turn an unrecognized custom route into a supplier name", () => {
    expect(currentProviderName(status({ kind: "externallyModified", at: "2026-09-01T09:42:30Z" }), profiles)).toBe(
      "未识别的供应商",
    );
  });

  it.each(["codex", "claude"] as const)("keeps %s provider identity despite model or settings differences", (app) => {
    const live = status({ kind: "profileChanged", profileName: "Historical supplier" });
    live.app = app;
    live.activeProfileId = "zhipu";
    expect(currentProviderName(live, [{ ...profiles[0], app }])).toBe("Zhipu GLM");
  });

  it("does not display the historical profile as the active supplier", () => {
    expect(currentProviderName(status({ kind: "profileChanged", profileName: "Historical supplier" }), profiles))
      .toBe("未识别的供应商");
  });
});
