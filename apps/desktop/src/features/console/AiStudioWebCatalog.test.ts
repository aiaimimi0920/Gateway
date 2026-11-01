import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";

it("keeps AI Studio Playground separate from Gemini Web and Canvas", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "aistudio-web")!;
  expect(template).toMatchObject({ preset: "aistudio-web-reverse", baseUrl: "https://aistudio.google.com" });
  expect(PROVIDER_CATALOG_TEMPLATES.some(({ id }) => id === "gemini-web")).toBe(true);
  expect(PROVIDER_CATALOG_TEMPLATES.some(({ id }) => id === "gemini-canvas")).toBe(true);
  const created = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, {
    templateId: template.id, providerId: template.providerId, providerLabel: template.labelZh,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
    supportedModels: [],
  });
  expect(created.providers[0]).toMatchObject({ preset: "aistudio-web-reverse", base_url: "https://aistudio.google.com",
    explicit_credentials_only: true, credentials: [], supported_models: [] });
  expect(created.providers[0]).not.toHaveProperty("api_key");
  expect(created.model_routes).toEqual([]);
});
