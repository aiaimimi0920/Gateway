import { expect, it } from "vitest";
import { DIM_TEMPLATES } from "./dimCatalog";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
it("offers two isolated DIM products with no guessed models, secrets or automatic paid probes", () => {
  for (const template of DIM_TEMPLATES) {
    expect(PROVIDER_CATALOG_TEMPLATES).toContain(template);
    const workflow = defaultCatalogWorkflow(template);
    expect(workflow.testPolicy.automaticEnabled).toBe(false);
    const p = providerDefinitionFromCatalogDraft(template, { templateId: template.id, providerId: template.id, providerLabel: template.id,
      vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl, supportedModels: [], workflow });
    expect(p).toMatchObject({ preset: template.id, auth_mode: "dim_user_oauth", explicit_credentials_only: true, supported_models: [] });
    expect(p).not.toHaveProperty("api_key"); expect(p).not.toHaveProperty("verified_models");
    expect(() => providerDefinitionFromCatalogDraft(template, { templateId: template.id, providerId: "dim", providerLabel: "dim", vendorKey: "dim", vendorName: "DIM", baseUrl: "https://evil.invalid/v1", supportedModels: [] })).toThrow();
  }
});
