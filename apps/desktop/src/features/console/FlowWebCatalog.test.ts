import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { googleWebAuthFamily } from "./GoogleWebAccountImport";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { TEMPLATE_ADAPTATION_RECEIPTS } from "./providerTemplateAdaptation";

it("exposes Google Flow independently without claiming accepted generation or adding model routes", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "flow-web");
  expect(template).toMatchObject({ preset: "flow-web", baseUrl: "https://flow.google.com", supportedModels: [] });
  expect(googleWebAuthFamily("flow-web")).toBe("flow-web");
  expect(googleWebAuthFamily("producer")).not.toBe("flow-web");
  const created = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
    templateId: template!.id, providerId: "flow-web", providerLabel: "Flow",
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [],
  });
  expect(created.providers[0]).toMatchObject({ preset: "flow-web", explicit_credentials_only: true, credentials: [] });
  expect(created.model_routes).toEqual([]);
  expect(defaultCatalogWorkflow(template!)).toMatchObject({ specialAdapter: "flow_web" });
  expect(TEMPLATE_ADAPTATION_RECEIPTS.some(receipt => receipt.templateId === "flow-web")).toBe(false);
});
