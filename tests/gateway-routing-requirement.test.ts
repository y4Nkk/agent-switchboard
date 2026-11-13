import assert from "node:assert/strict";
import { test } from "node:test";
import { requiresGateway, type GatewayProfileInput } from "../src/lib/protocol.ts";

const direct: GatewayProfileInput = { app: "codex", routeMode: "custom", upstreamProtocol: "responses",
  baseUrl: "https://provider.invalid/v1", responsesOptions: { requestMode: "standard" }, connection: {}, authentication: "bearer" };

test("Codex native connections become gateway routes for model aliases or subagent targets", () => {
  assert.equal(requiresGateway(direct), false);
  assert.equal(requiresGateway({ ...direct, modelRoutes: [{ clientModel: "local", upstreamModel: "remote" }] }), true);
  assert.equal(requiresGateway({ ...direct, subagentRoute: { profileId: "11111111-1111-4111-8111-111111111111", model: "fast" } }), true);
});

test("Claude fixed destinations require the gateway while local model preferences retain direct routing", () => {
  const claude: GatewayProfileInput = { ...direct, app: "claude", upstreamProtocol: "anthropicMessages", responsesOptions: null,
    modelOptions: { kind: "claude", primaryOneM: false, availableModels: null,
      routes: { haiku: null, sonnet: null, opus: null, fable: null,
        subagent: { mode: "primary", model: "fast", oneM: false } } } };
  assert.equal(requiresGateway(claude), false);
  if (claude.modelOptions?.kind !== "claude") throw new Error("Wrong fixture");
  assert.equal(requiresGateway({ ...claude, modelOptions: { ...claude.modelOptions,
    routes: { ...claude.modelOptions.routes,
      subagent: { mode: "provider", profileId: "11111111-1111-4111-8111-111111111111", model: "fast", oneM: false } } } }), true);
});
