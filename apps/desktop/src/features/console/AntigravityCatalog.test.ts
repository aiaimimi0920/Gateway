import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";
import { readProviderRequestClientProfile, writeRequestClientProfile } from "./requestClientProfile";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

it("creates an independent OAuth provider with an executor-owned Antigravity fingerprint", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(t => t.id === "antigravity-oauth")!;
  expect(template).toBeDefined();
  expect(template.supportedModels).toEqual([]);
  const workflow = defaultCatalogWorkflow(template);
  expect(workflow.specialAdapter).toBe("antigravity_oauth");
  const doc = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, {
    templateId: template.id, providerId: "agy", providerLabel: "Antigravity", vendorKey: template.vendorKey,
    vendorName: template.vendorName, baseUrl: template.baseUrl, supportedModels: [], workflow,
  });
  const provider = doc.providers[0] as Record<string, unknown>;
  expect(provider).toMatchObject({ preset: "antigravity-oauth", explicit_credentials_only: true,
    call_flow: { special_adapter: "antigravity_oauth", fingerprint_adapter: "antigravity" } });
  expect(readProviderRequestClientProfile(provider)).toBe("antigravity");
  expect(writeRequestClientProfile(provider, "codex_cli")).toBe(false);
  expect(templateAdaptationComplete(template.id)).toBe(false);
});
