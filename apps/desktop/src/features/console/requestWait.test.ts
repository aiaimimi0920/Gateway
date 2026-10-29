import { describe, expect, it } from "vitest";
import { writeRecoveryWait, type RecoveryWaitChoice } from "./requestWait";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";

describe("provider recovery waiting", () => {
  it.each(["300", "600", "900"] as RecoveryWaitChoice[])("writes a bounded %s-second policy", (choice) => {
    const provider: Record<string, unknown> = {};
    writeRecoveryWait(provider, choice);
    expect(provider.request_wait).toEqual({ mode: "bounded", total_timeout_seconds: Number(choice) });
    expect(provider.extra_body).toBeUndefined();
  });
  it("keeps automatic defaults absent and supports long wait and explicit opt-out", () => {
    const provider: Record<string, unknown> = {};
    writeRecoveryWait(provider, "client_connected");
    expect(provider.request_wait).toEqual({ mode: "client_connected" });
    writeRecoveryWait(provider, "disabled");
    expect(provider.request_wait).toEqual({ mode: "disabled" });
    writeRecoveryWait(provider);
    expect(provider.request_wait).toBeUndefined();
  });
  it("persists the selection through normal provider creation", () => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((t) => t.id === "chatgpt")!;
    const provider = providerDefinitionFromCatalogDraft(template, { templateId: template.id, providerId: "fixture",
      providerLabel: "fixture", vendorKey: template.vendorKey, vendorName: template.vendorName,
      baseUrl: template.baseUrl, supportedModels: [], recoveryWait: "900" });
    expect(provider.request_wait).toEqual({ mode: "bounded", total_timeout_seconds: 900 });
  });
});
