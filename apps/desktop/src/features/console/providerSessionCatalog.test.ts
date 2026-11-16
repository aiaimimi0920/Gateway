import { describe, expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";
import { SESSION_PRODUCTS } from "./providerSessionProducts";
import { testProviderContext } from "./credentialTestPolicyDocument";

describe("session products are independent normal empty-pool templates", () => {
  it.each(Object.entries(SESSION_PRODUCTS))("creates %s with its exact executor and no invented account/model", (id, product) => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === id);
    expect(template).toBeDefined();
    const workflow = defaultCatalogWorkflow(template!);
    const document = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
      templateId: id, providerId: `renamed-${id}`, providerLabel: id, vendorKey: product.vendorKey, vendorName: product.vendorName,
      baseUrl: product.baseUrl, supportedModels: [], workflow,
    });
    expect(document.providers[0]).toMatchObject({ id: `renamed-${id}`, preset: product.preset, base_url: product.baseUrl,
      explicit_credentials_only: true, credentials: [], supported_models: [],
      call_flow: { call_adapter: "auto", fingerprint_adapter: "session", special_adapter: product.special },
      auto_refill_enabled: false, auto_prune_enabled: false, test_policy: { automaticEnabled: false } });
    expect(testProviderContext(document, `renamed-${id}`)?.accounts).toEqual([]);
    expect(workflow.health.enabled).toBe(false);
    expect(templateAdaptationComplete(id)).toBe(false);
    if (["chataibot-web", "lumalabs-web", "suno-web", "udio-web"].includes(id)) {
      expect(workflow.testPolicy.cases).toHaveLength(1);
      expect(workflow.testPolicy.cases[0].expectedAnswer).toBeNull();
      expect(workflow.testPolicy.cases[0].prompt).not.toContain("Reply with only OK");
    }
  });
  it("contains each template exactly once and keeps new brands outside the Gemini insertion block", () => {
    const ids = PROVIDER_CATALOG_TEMPLATES.map(({ id }) => id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.indexOf("grok-web")).toBe(ids.indexOf("xai-api") + 1);
    expect(ids.indexOf("suno-web")).toBeGreaterThan(ids.indexOf("custom-api-provider"));
    expect(ids).not.toContain("freebuff-api");
    expect(ids).toContain("grok-oauth");
    const build = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "grok-oauth");
    expect(build).toMatchObject({ preset: "grok-oauth", baseUrl: "https://cli-chat-proxy.grok.com/v1", supportedModels: [] });
    expect(templateAdaptationComplete("grok-oauth")).toBe(false);
  });
});
