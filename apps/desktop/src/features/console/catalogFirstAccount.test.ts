import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { catalogFirstAccountSecret, supportsCatalogApiKey } from "./catalogFirstAccount";
import { buildCredentialSecretPatches } from "./credentialDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";

const template = PROVIDER_CATALOG_TEMPLATES.find(t => t.custom)!;
const draft: ProviderCatalogDraft = { templateId: template.id, providerId: "relay", providerLabel: "Relay", vendorKey: "relay", vendorName: "Relay", baseUrl: "https://relay.example/v1", supportedModels: ["model"], credentialId: "account", accountName: "First", apiKey: "synthetic-test-key" };
const empty: ConsoleRouteDocument = { providers: [], model_routes: [], aliases: {} };
it("creates provider, routing and account together but keeps secrets outside route JSON", () => {
  const next = addCatalogProvider(empty, template, draft);
  expect(next.providers[0]).toMatchObject({ credentials: [{ id: "account", account_name: "First", enabled: true }] });
  expect(next.model_routes).toEqual([{ pattern: "model", provider_ids: ["relay"] }]);
  expect(JSON.stringify(next)).not.toContain(draft.apiKey);
  next.providers.unshift({ id: "other" });
  const patches = buildCredentialSecretPatches({ activeDocument: empty, draftDocument: next, activeSecrets: [], credentialSecretEdits: [catalogFirstAccountSecret(draft)!] });
  expect(patches).toContainEqual({ path: "/providers/1/credentials/0/api_key", operation: "replace", value: draft.apiKey });
  expect(empty.providers).toEqual([]);
});
it("allows provider-only creation and rejects missing keys or duplicate accounts", () => {
  expect(addCatalogProvider(empty, template, { ...draft, credentialId: undefined, apiKey: undefined }).providers[0]).toMatchObject({ credentials: [] });
  expect(() => addCatalogProvider(empty, template, { ...draft, apiKey: " " })).toThrow("API Key");
  const original = { ...empty, providers: [{ id: "other", credentials: [{ id: "account" }] }] };
  expect(() => addCatalogProvider(original, template, draft)).toThrow();
  expect(original.providers).toHaveLength(1);
});
it("does not turn typed vendor authorization into a generic key field", () => {
  for (const id of ["lumalabs-mcp", "vertex-oauth", "kiro-oauth", "bedrock-api", "lumalabs-api"]) {
    const vendor = PROVIDER_CATALOG_TEMPLATES.find(t => t.id === id)!;
    expect(supportsCatalogApiKey(vendor)).toBe(false);
    expect(() => addCatalogProvider(empty, vendor, { ...draft, templateId: id, baseUrl: vendor.baseUrl })).toThrow();
  }
});
