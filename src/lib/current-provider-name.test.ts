import { describe, expect, it } from "vitest";
import type { ConfigFileStatus } from "../api/client";
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
    matchStatus,
    lastSwitch: null,
  };
}

describe("currentProviderName", () => {
  it("uses the active matching profile name over the custom connection mode", () => {
    expect(
      currentProviderName(
        status({ kind: "matchesProfile", profileId: "zhipu", profileName: "Zhipu GLM" }),
      ),
    ).toBe("Zhipu GLM");
  });

  it("does not turn an unrecognized custom route into a supplier name", () => {
    expect(currentProviderName(status({ kind: "externallyModified", at: "2026-09-01T09:42:30Z" }))).toBe(
      "未识别的供应商",
    );
  });
});
