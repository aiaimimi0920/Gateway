import { expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { sessionProductForPreset } from "./providerSessionProducts";

it("adds an empty Accio OAuth pool without merging legacy accounts or inventing routes", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "accio-oauth");
  expect(template).toBeDefined();
  if (!template) throw new Error("Missing Accio template");
  const original: ConsoleRouteDocument = { providers: [{ id: "legacy", preset: "accio", credentials: [{ id: "old", api_key: "synthetic" }] }], model_routes: [], aliases: {} };
  const before = structuredClone(original);
  const draft = { templateId: template.id, providerId: "renamed-accio", providerLabel: "Accio", vendorKey: template.vendorKey,
    vendorName: template.vendorName, baseUrl: template.baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(template) };
  const next = addCatalogProvider(original, template, draft);
  expect(original).toEqual(before);
  expect(next.providers[0]).toEqual(original.providers[0]);
  expect(next.model_routes).toEqual([]);
  expect(next.providers[1]).toMatchObject({ id: "renamed-accio", preset: "accio", base_url: "https://phoenix-gw.alibaba.com",
    explicit_credentials_only: true, credentials: [], supported_models: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "session", special_adapter: "accio" } });
  expect(sessionProductForPreset("accio")).toBeUndefined();
  expect(() => providerDefinitionFromCatalogDraft(template, { ...draft, baseUrl: "https://attacker.invalid" })).toThrow();
});
