import { describe, expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogTemplate } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { catalogHealthAdapter, defaultCatalogWorkflow } from "./providerCatalogWorkflow";

function create(template: ProviderCatalogTemplate, enabled?: boolean) {
  const workflow = defaultCatalogWorkflow(template);
  if (enabled !== undefined) workflow.health.enabled = enabled;
  return addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, {
    templateId: template.id, providerId: template.providerId, providerLabel: template.labelEn,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
    supportedModels: [...template.supportedModels], workflow,
  }).providers[0];
}

describe("provider template site-health defaults", () => {
  const supported = PROVIDER_CATALOG_TEMPLATES.filter((template) => catalogHealthAdapter(template));
  it.each(supported)("enables supported $id monitoring without account tests or secrets", (template) => {
    const workflow = defaultCatalogWorkflow(template);
    expect(workflow.health).toMatchObject({ enabled: true, adapter: catalogHealthAdapter(template),
      mode: "advisory", unknown_policy: "allow", refresh_interval_secs: 300, max_age_secs: 900,
      allow_private_network: false, auth: { kind: "none" } });
    expect(workflow.testPolicy.automaticEnabled).toBe(false);
    expect(create(template)).toMatchObject({ site_health: workflow.health,
      explicit_credentials_only: true, credentials: [], test_policy: { automaticEnabled: false } });
  });
  it.each(supported)("honors explicitly disabling $id monitoring", (template) => {
    expect(create(template, false)).not.toHaveProperty("site_health");
    expect(defaultCatalogWorkflow(template).health.enabled).toBe(true);
  });
  it("does not invent official health support for unmatched products", () => {
    const unsupported = PROVIDER_CATALOG_TEMPLATES.filter((template) => !catalogHealthAdapter(template));
    expect(unsupported.map((template) => template.id)).toContain("websearchapi");
    for (const template of unsupported) {
      expect(defaultCatalogWorkflow(template).health.enabled).toBe(false);
      expect(create(template)).not.toHaveProperty("site_health");
    }
  });
});
