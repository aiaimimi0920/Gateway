import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { TEMPLATE_ADAPTATION_RECEIPTS, templateAdaptationComplete } from "./providerTemplateAdaptation";

it("reuses Producer HTTP music without exposing image/video or a text IQ probe", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "flowmusic-web");
  expect(template).toBeDefined();
  const workflow = defaultCatalogWorkflow(template!);
  const created = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
    templateId: template!.id, providerId: template!.providerId, providerLabel: template!.labelZh,
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [...template!.supportedModels], workflow,
  });
  expect(created.providers[0]).toMatchObject({ preset: "producer", base_url: "https://www.flowmusic.app",
    supported_models: ["producer:standard"], supported_protocol_families: ["producer_music"],
    explicit_credentials_only: true, credentials: [], execution_mode: "direct_http",
    call_flow: { call_adapter: "auto", fingerprint_adapter: "session", special_adapter: "flowmusic_web" } });
  expect(workflow.testPolicy.automaticEnabled).toBe(false);
  expect(workflow.testPolicy.models).toEqual(["producer:standard"]);
  expect(workflow.testPolicy.cases).toHaveLength(1);
  expect(workflow.testPolicy.cases[0].expectedAnswer).toBeNull();
  expect(workflow.testPolicy.cases[0].prompt).toContain("instrumental music");
  expect(workflow.health.enabled).toBe(false);
  expect(templateAdaptationComplete("flowmusic-web")).toBe(true);
  expect(TEMPLATE_ADAPTATION_RECEIPTS.find(({ templateId }) => templateId === "flowmusic-web")).toMatchObject({
    evidencePath: "docs/progress/2026-11-01-flowmusic-web.md", flowVersion: 1,
  });
});
