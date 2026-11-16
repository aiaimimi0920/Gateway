import { expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";

it("creates an independent empty Build pool without changing API/Web materials or default routes", () => {
  const original: ConsoleRouteDocument = { providers: [
    { id: "existing-api", preset: "xai-api", credentials: [{ id: "api-account", api_key: "synthetic-key" }] },
    { id: "existing-web", preset: "grok", credentials: [{ id: "web-account", api_key: "synthetic-cookie" }] },
  ], model_routes: [{ pattern: "existing-model", provider_ids: ["existing-api"] }], aliases: {} };
  const previous = structuredClone(original);
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "grok-oauth")!;
  const next = addCatalogProvider(original, template, {
    templateId: template.id, providerId: "renamed-build", providerLabel: "Build", vendorKey: "xai", vendorName: "xAI",
    baseUrl: template.baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(template),
  });
  expect(original).toEqual(previous);
  expect(next.providers.slice(0, 2)).toEqual(original.providers);
  expect(next.model_routes).toEqual(original.model_routes);
  expect(next.providers[2]).toMatchObject({ id: "renamed-build", preset: "grok-oauth", base_url: "https://cli-chat-proxy.grok.com/v1",
    explicit_credentials_only: true, credentials: [], supported_models: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
    test_policy: { automaticEnabled: false },
  });
  expect(buildRouteAccountCatalog(next).accounts.filter(({ providerId }) => providerId === "renamed-build")).toEqual([]);
  expect(PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "xai-api")).toMatchObject({ providerId: "xAI-api", preset: "xai-api" });
});
