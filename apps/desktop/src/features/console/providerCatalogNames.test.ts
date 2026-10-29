import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";

it.each(PROVIDER_CATALOG_TEMPLATES)("$id uses a suffixed default provider ID and labels", (template) => {
  expect(template.providerId).toMatch(/-(oauth|api|web)$/);
  expect(template.labelZh).toBe(template.providerId);
  expect(template.labelEn).toBe(template.providerId);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, {
    templateId: template.id, providerId: template.providerId, providerLabel: template.providerId,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
    supportedModels: [], workflow: defaultCatalogWorkflow(template),
  });
  expect(result.providers[0]).toMatchObject({ id: template.providerId, label: template.providerId, credentials: [] });
  if (template.preset) expect(result.providers[0]).toMatchObject({ preset: template.preset });
});
it("keeps provider IDs unique without rewriting existing providers or route references", () => {
  const ids = PROVIDER_CATALOG_TEMPLATES.map((template) => template.providerId);
  expect(new Set(ids).size).toBe(ids.length);
  const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "cohere-chat")!;
  const document = { providers: [{ id: "cohere", credentials: [{ id: "old-account" }] }],
    model_routes: [{ pattern: "old-model", provider_ids: ["cohere"] }], aliases: {} };
  const result = addCatalogProvider(document, template, {
    templateId: template.id, providerId: template.providerId, providerLabel: template.providerId,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
    supportedModels: [], workflow: defaultCatalogWorkflow(template),
  });
  expect(result.providers[0]).toEqual(document.providers[0]);
  expect(result.model_routes).toEqual(document.model_routes);
  expect(result.providers[1]).toMatchObject({ id: "cohere-api" });
});
