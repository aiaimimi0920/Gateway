import { describe, expect, it } from "vitest";
import { parseProviderSessionMaterial, SESSION_MATERIAL_MAX_BYTES } from "./providerSessionMaterial";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";
import { sessionProductForPreset, SESSION_PRODUCTS } from "./providerSessionProducts";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const sample = (product = "grok-web") => ({ product, token: "synthetic-session-token" });
describe("product-bound session material", () => {
  it.each(Object.keys(SESSION_PRODUCTS))("accepts the actual minimum material for %s", (product) => {
    const source = { ...sample(product), ...(product === "suno-web" ? { cookieHeader: "__session=synthetic-session-token; other=fixture" } : {}),
      ...(product === "perplexity-web" ? { userAgent: "synthetic-agent" } : {}),
      ...(product === "lumalabs-web" ? { realmId: "board-fixture" } : {}), ...(product === "freebuff-web" ? { agentId: "agent-fixture" } : {}) };
    const material = parseProviderSessionMaterial(JSON.stringify(source), product as keyof typeof SESSION_PRODUCTS);
    expect(material.product).toBe(product);
    expect(material.token).toBe("synthetic-session-token");
  });
  it("extracts a Cookie value rather than using the complete header as a token", () => {
    const material = parseProviderSessionMaterial(JSON.stringify({ product: "grok-web", cookieHeader: "sso=fixture; sso-rw=fixture; cf_clearance=other" }), "grok-web");
    expect(material.token).toBe("fixture");
    expect(material.cookieHeader).toContain("cf_clearance");
  });
  it("preserves Udio split cookies without inventing a bearer token", () => {
    const material = parseProviderSessionMaterial(JSON.stringify({ product: "udio-web", cookieHeader: "sb-ssr-production-auth-token.0=part-a; sb-ssr-production-auth-token.1=part-b" }), "udio-web");
    expect(material.token).toBeUndefined();
    expect(material.cookieHeader).toContain(".1=part-b");
  });
  it.each([
    { ...sample(), product: "qwen-web" }, { ...sample(), token: "sso=bad; x=y" },
    { ...sample(), cookieHeader: "sso=other" }, { ...sample(), cookieHeader: "sso=a; sso=b" },
    { ...sample(), cookieHeader: "x=y\r\nAuthorization: bad" }, { ...sample(), expiresAt: "2000-01-01T00:00:00Z" },
    { ...sample(), expiresAt: "2099-02-31T00:00:00Z" }, { ...sample(), refreshToken: "do-not-echo" },
    { ...sample(), baseUrl: "https://evil.test" }, { ...sample(), headers: { Authorization: "do-not-echo" } },
    { ...sample(), authSeed: { password: "do-not-echo" } }, { token: "do-not-echo" }, [], null,
    { ...sample(), token: "sso=do-not-echo" }, { ...sample(), userAgent: "bad\u0085value" }, { ...sample(), expiresAt: "2099-01-01T24:00:00Z" },
  ])("rejects wrong-channel, stale, unsafe or unrecognized input", (source) => {
    expect(() => parseProviderSessionMaterial(JSON.stringify(source), "grok-web")).toThrow();
    try { parseProviderSessionMaterial(JSON.stringify(source), "grok-web"); } catch (error) { expect(String(error)).not.toContain("do-not-echo"); }
  });
  it.each(["suno-web", "lumalabs-web", "freebuff-web"] as const)("requires product-specific material for %s", (product) => {
    expect(() => parseProviderSessionMaterial(JSON.stringify(sample(product)), product)).toThrow();
  });
  it("bounds UTF-8 input bytes and does not accept raw invalid JSON", () => {
    expect(() => parseProviderSessionMaterial("中".repeat(SESSION_MATERIAL_MAX_BYTES / 2), "grok-web")).toThrow("128 KiB");
    expect(() => parseProviderSessionMaterial("{do-not-echo", "grok-web")).toThrow("JSON");
  });
  it("uses saved preset aliases, never a mutable provider ID", () => {
    expect(sessionProductForPreset("qwen-webui-replay-live")).toBe("qwen-web");
    expect(sessionProductForPreset("xai-api")).toBeUndefined();
    expect(sessionProductForPreset("grok-web")).toBeUndefined();
  });
});

describe("session import uses the normal sanitized document and SecretPatch owner", () => {
  const document = () => routeDocument([{ id: "renamed", preset: "lumalabs", base_url: "https://app.lumalabs.ai", credentials: [{ id: "account", headers: { Accept: "application/json" } }] }]);
  const material = () => parseProviderSessionMaterial(JSON.stringify({ ...sample("lumalabs-web"), realmId: "board-fixture", cookieHeader: "wos-session=synthetic-session-token" }), "lumalabs-web");
  it("keeps token and Cookie out of editor JSON, preserves metadata and rebases by IDs", () => {
    const original = document();
    const result = applyProviderSessionMaterial(original, "renamed", "account", material());
    expect(JSON.stringify(result.document)).not.toContain("synthetic-session-token");
    expect(result.document.providers[0]).toMatchObject({ credentials: [{ extra_body: { realmId: "board-fixture" }, headers: { Accept: "application/json" } }] });
    result.document.providers.unshift({ id: "other", credentials: [] });
    const patches = buildCredentialSecretPatches({ activeDocument: original, draftDocument: result.document, activeSecrets: [], credentialSecretEdits: result.secretEdits });
    expect(patches).toContainEqual({ path: "/providers/1/credentials/0/api_key", operation: "replace", value: "synthetic-session-token" });
    expect(patches).toContainEqual({ path: "/providers/1/credentials/0/headers/Cookie", operation: "replace", value: "wos-session=synthetic-session-token" });
    expect(original.providers[0]).not.toHaveProperty("credentials.0.extra_body");
  });
  it.each([
    { preset: "xai-api" }, { base_url: "https://app.lumalabs.ai.evil.test" }, { base_url: "http://app.lumalabs.ai" },
    { base_url: "https://user:pass@app.lumalabs.ai" }, { base_url: "https://app.lumalabs.ai/redirect?to=evil" },
    { call_flow: { special_adapter: "grok_web" } }, { adapter: "openai_compatible" },
  ])("rejects mismatched product/audience/config without changing the input", (overrides) => {
    const input = document(); Object.assign(input.providers[0]!, overrides);
    const before = JSON.stringify(input);
    expect(() => applyProviderSessionMaterial(input, "renamed", "account", material())).toThrow();
    expect(JSON.stringify(input)).toBe(before);
  });
  it("checks credential endpoint overrides too", () => {
    const input = document(); (input.providers[0] as any).credentials[0].base_url = "https://evil.test";
    expect(() => applyProviderSessionMaterial(input, "renamed", "account", material())).toThrow();
  });
  it.each(["destination", "product", "realm", "flow", "shared-auth"])("checks pending binding again at commit: %s", (change) => {
    const input = document(); const result = applyProviderSessionMaterial(input, "renamed", "account", material());
    const provider = result.document.providers[0] as Record<string, any>;
    if (change === "destination") provider.base_url = "https://evil.test";
    if (change === "product") provider.preset = "xai-api";
    if (change === "realm") provider.credentials[0].extra_body.realmId = "other-board";
    if (change === "flow") provider.call_flow = { special_adapter: "grok_web" };
    expect(() => buildCredentialSecretPatches({ activeDocument: input, draftDocument: result.document,
      activeSecrets: change === "shared-auth" ? [{ path: "/providers/0/headers/COOKIE", configured: true }] : [], credentialSecretEdits: result.secretEdits })).toThrow();
  });
  it("does not mix a new Qwen session with an old recovery identity", () => {
    const input = routeDocument([{ id: "q", preset: "qwen-web-chat", base_url: "https://chat.qwen.ai/api/v1", credentials: [{ id: "a", extra_body: { authSeed: { email: "fixture@example.test" } } }] }]);
    const parsed = parseProviderSessionMaterial(JSON.stringify(sample("qwen-web")), "qwen-web");
    expect(() => applyProviderSessionMaterial(input, "q", "a", parsed)).toThrow("恢复 seed");
  });
  it("clears api_key for Udio split cookies and does not preserve old token or Cookie", () => {
    const input = routeDocument([{ id: "u", preset: "udio", base_url: "https://www.udio.com", credentials: [{ id: "a" }] }]);
    const parsed = parseProviderSessionMaterial(JSON.stringify({ product: "udio-web", cookieHeader: "sb-ssr-production-auth-token.0=a; sb-ssr-production-auth-token.1=b" }), "udio-web");
    const result = applyProviderSessionMaterial(input, "u", "a", parsed);
    expect(result.secretEdits).toContainEqual(expect.objectContaining({ field: "api_key", operation: "clear" }));
  });
  it("clears stale Cookie and Authorization paths with each original casing", () => {
    const input = document(); const result = applyProviderSessionMaterial(input, "renamed", "account", material());
    const patches = buildCredentialSecretPatches({ activeDocument: input, draftDocument: result.document,
      activeSecrets: ["cookie", "COOKIE", "authorization"].map((key) => ({ path: `/providers/0/credentials/0/headers/${key}`, configured: true })), credentialSecretEdits: result.secretEdits });
    for (const key of ["cookie", "COOKIE", "authorization"]) expect(patches).toContainEqual({ path: `/providers/0/credentials/0/headers/${key}`, operation: "clear" });
    expect(patches).toContainEqual({ path: "/providers/0/credentials/0/headers/Cookie", operation: "replace", value: "wos-session=synthetic-session-token" });
  });
});
