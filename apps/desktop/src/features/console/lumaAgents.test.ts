// Product validation accepts nullable management JSON but never retargets staged materials.
import { expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { LUMA_AGENTS_BASE as base, lumaAgentsMaterialTarget, normalizeLumaAgentsDraft,
  validateLumaAgentsProvider, validateLumaAgentsStaging } from "./lumaAgents";
import { providerBrandLabel } from "./providerBrandLabel";
import { providerCardVisual } from "./providerCardVisual";

const provider = () => ({ id: "pool", preset: "lumalabs-api", base_url: base });
const draft = (): CredentialDialogValue => ({ providerId: "pool", credentialId: "key", accountName: "", enabled: true,
  baseUrl: "", apiKeyOperation: "replace", apiKeyValue: "luma-api-synthetic", supportedModelsText: "uni-1" });
const document = (credential: object): ConsoleRouteDocument => ({ providers: [{ ...provider(), credentials: [{ id: "key", ...credential }] }], model_routes: [], aliases: {} });
it("accepts explicit native identity, nullable optional fields and aliases to supported targets", () => {
  expect(validateLumaAgentsProvider({ ...provider(), adapter: "lumalabs_agents_compatible", protocol_profile: "lumalabs_agents_api",
    protocol_family: "lumalabs_agents_media", request_client_profile: "auto", auth_mode: "bearer", auth_header_name: "Authorization",
    execution_mode: "direct_http", endpoint_execution_modes: { image: "direct_http" }, headers: {}, extra_body: {},
    refresh_token: null, session_auth: null, call_flow: null, supported_models: [], model_map: { alias: "uni-1" },
    model_map_targets: { "my-video": ["ray-3.2"] } }, { base_url: `${base}/`, auth_mode: null })).toEqual({ product: "lumalabs-api", baseUrl: base, authMode: "bearer" });
});
it.each([{ headers: { Authorization: "foreign" } }, { refresh_token: "foreign" }, { discovery: {} }, { discovery_job: {} },
  { model_catalog: {} }, { session_auth: {} }, { auth_token: "foreign" }, { request_client_profile: "codex_cli" },
  { execution_mode: "browser_backed" }, { endpoint_execution_modes: { image: "browser_backed" } },
  { extra_body: { realm: "web" } }, { balance_path: "/balance" }, { call_flow: { special_adapter: "none" } },
  { model_selection: { fetched: ["uni-1"], manual: [] } }, { supported_models: ["ray3.14"] }])("checks the final account surface: %j", (patch) => {
  expect(() => lumaAgentsMaterialTarget(document(patch), "pool", "key")).toThrow("Luma Agents");
});
it.each(["https://agents.lumalabs.ai:444/v1", `${base}?x=1`, "https://user@agents.lumalabs.ai/v1", "http://agents.lumalabs.ai/v1",
  "https://app.lumalabs.ai", `${base}/other`])("does not capture a changed Key audience %s", (base_url) => {
  expect(() => lumaAgentsMaterialTarget(document({ base_url }), "pool", "key")).toThrow("固定");
});
it("cannot hide a foreign provider flow behind an account override", () => {
  const doc = document({ call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" } });
  Object.assign(doc.providers[0] as object, { call_flow: { special_adapter: "none" } });
  expect(() => lumaAgentsMaterialTarget(doc, "pool", "key")).toThrow();
});
it("accepts exact Key boundaries and the ordinary edit Keep/Clear contract", () => {
  for (const key of ["luma-api-x", `luma-api-${"x".repeat(4087)}`]) expect(normalizeLumaAgentsDraft(provider(), { ...draft(), apiKeyValue: key }, false).apiKeyValue).toBe(key);
  for (const apiKeyOperation of ["keep", "clear"] as const) {
    expect(normalizeLumaAgentsDraft(provider(), { ...draft(), apiKeyOperation, apiKeyValue: "" }, true).apiKeyOperation).toBe(apiKeyOperation);
    expect(() => normalizeLumaAgentsDraft(provider(), { ...draft(), apiKeyOperation }, false)).toThrow("静态");
  }
});
it.each(["luma-api-中文", "luma-api-\u007f", "luma-api-\nsecond", "wos-session"])("rejects non-Key material %s", (apiKeyValue) => {
  expect(() => normalizeLumaAgentsDraft(provider(), { ...draft(), apiKeyValue }, false)).toThrow("Key");
});
it("refuses another product's material before controller-side staging", () => {
  expect(() => normalizeLumaAgentsDraft(provider(), { ...draft(), sessionMaterial: { product: "lumalabs-web" } as any }, false)).toThrow("会话");
  expect(() => normalizeLumaAgentsDraft(provider(), { ...draft(), discovery: { models: [] } as any }, false)).toThrow("目录");
});
it("rejects deleted identities instead of rebinding them by array position", () => {
  const doc = document({}), binding = lumaAgentsMaterialTarget(doc, "pool", "key");
  expect(() => validateLumaAgentsStaging(doc, "pool", "key", binding)).not.toThrow();
  (doc.providers[0] as Record<string, unknown>).credentials = [{ id: "other" }];
  expect(() => validateLumaAgentsStaging(doc, "pool", "key", binding)).toThrow("账号");
});
it("brands a renamed pool from the actual preset and categorizes it as media", () => {
  expect(providerBrandLabel({ providerId: "renamed", providerPreset: "lumalabs-api" })).toBe("Luma Agents · Renamed");
  expect(providerCardVisual({ providerId: "renamed", providerPreset: "lumalabs-api", providerLabel: "Luma Agents" }).category).toBe("media");
});
it("keeps Agents branding when a user reuses the old Web pool ID", () => {
  expect(providerBrandLabel({ providerId: "lumalabs", providerPreset: "lumalabs-api" })).toBe("Luma Agents");
});
it("does not turn the media product into a browser product through a mutable pool ID", () => {
  expect(providerCardVisual({ providerId: "qwen-web-chat", providerPreset: "lumalabs-api", providerLabel: "Renamed" }).category).toBe("media");
});
