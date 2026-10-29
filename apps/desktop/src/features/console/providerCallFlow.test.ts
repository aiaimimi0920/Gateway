import { describe, expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft, type ProviderCatalogDraft } from "./providerCatalog";
import { defaultCatalogWorkflow, type CallAdapter } from "./providerCatalogWorkflow";
import { writeRequestClientProfile } from "./requestClientProfile";

const custom = PROVIDER_CATALOG_TEMPLATES.find((template) => template.custom)!;
function draft(): ProviderCatalogDraft {
  return { templateId: custom.id, providerId: "fixture", providerLabel: "fixture", vendorKey: "fixture", vendorName: "fixture",
    baseUrl: "https://fixture.test", supportedModels: [], workflow: defaultCatalogWorkflow(custom) };
}

describe("independent runtime call flow", () => {
  it.each(["auto", "openai", "anthropic", "gemini", "cohere"] as CallAdapter[])("persists format %s independently of fingerprint", (adapter) => {
    const value = draft(); value.workflow!.callAdapter = adapter; value.requestClientProfile = "workbuddy";
    const provider = providerDefinitionFromCatalogDraft(custom, value);
    expect(provider.call_flow).toEqual({ call_adapter: adapter, fingerprint_adapter: "workbuddy", special_adapter: "none" });
    expect(provider.request_client_profile).toBe("workbuddy");
  });
  it("uses executor-owned identity for Basispoints instead of claiming the API profile applies", () => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "chatgpt")!;
    const value = draft(); value.workflow = defaultCatalogWorkflow(template); value.workflow.specialAdapter = "basispoints";
    value.requestClientProfile = "codex_cli";
    const provider = providerDefinitionFromCatalogDraft(template, value);
    expect(provider.call_flow).toEqual({ call_adapter: "auto", fingerprint_adapter: "session", special_adapter: "basispoints" });
    expect(provider.codex_basispoints_enabled).toBe(true);
    expect(provider).not.toHaveProperty("request_client_profile");
  });
  it("keeps existing fingerprint editing effective without resetting other flow selections", () => {
    const before = { call_adapter: "anthropic", fingerprint_adapter: "auto", special_adapter: "none" };
    const provider: Record<string, unknown> = { call_flow: before };
    writeRequestClientProfile(provider, "claude_cli");
    expect(provider).toMatchObject({ request_client_profile: "claude_cli", call_flow: { ...before, fingerprint_adapter: "claude_cli" } });
    expect(before.fingerprint_adapter).toBe("auto");
    writeRequestClientProfile(provider, "auto");
    expect(provider).not.toHaveProperty("request_client_profile");
    expect(provider.call_flow).toEqual(before);
  });
  it("does not inject flow into an existing legacy fingerprint write", () => {
    const provider: Record<string, unknown> = {};
    writeRequestClientProfile(provider, "codex_cli");
    expect(provider).toEqual({ request_client_profile: "codex_cli" });
  });
  it("does not introduce a flow when serializing an old catalog draft", () => {
    const value = draft(); delete value.workflow;
    expect(providerDefinitionFromCatalogDraft(custom, value)).not.toHaveProperty("call_flow");
  });
});
