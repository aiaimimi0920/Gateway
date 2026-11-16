import { describe, expect, it } from "vitest";
import { providerDefinitionFromCatalogDraft, type ProviderCatalogTemplate } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";
import { readProviderRequestClientProfile, writeRequestClientProfile } from "./requestClientProfile";

// Product executors are independent of how their session/token was obtained.
const PRODUCTS = [
  ["accio", "accio"], ["kiro", "kiro"], ["freebuff", "freebuff"],
  ["grok_web", "grok"], ["qwen_web", "qwen-web-chat"],
  ["chataibot_web", "chataibot"], ["lumalabs_web", "lumalabs"],
  ["suno_web", "suno"], ["udio_web", "udio"],
] as const;

function template(preset: string): ProviderCatalogTemplate {
  return { id: `fixture-${preset}`, providerId: `fixture-${preset}`, labelZh: preset, labelEn: preset,
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native", preset,
    vendorKey: "fixture", vendorName: "fixture", baseUrl: "https://fixture.invalid", supportedModels: [] };
}

describe("product-owned call flow", () => {
  it.each(PRODUCTS)("binds %s to preset %s without adding accounts or claiming API fingerprints", (special, preset) => {
    const entry = template(preset);
    const workflow = defaultCatalogWorkflow(entry);
    expect(workflow.specialAdapter).toBe(special);
    expect(workflow.callAdapter).toBe("auto");
    const draft = { templateId: entry.id, providerId: "renamed-provider", providerLabel: "fixture",
      vendorKey: entry.vendorKey, vendorName: entry.vendorName, baseUrl: entry.baseUrl,
      supportedModels: [], workflow, requestClientProfile: "codex_cli" as const };
    const provider = providerDefinitionFromCatalogDraft(entry, draft);
    expect(provider.preset).toBe(preset);
    expect(provider.call_flow).toEqual({ call_adapter: "auto", fingerprint_adapter: "session", special_adapter: special });
    expect(provider).not.toHaveProperty("request_client_profile");
    expect(readProviderRequestClientProfile(provider)).toBe("session");
    const before = structuredClone(provider);
    expect(writeRequestClientProfile(provider, "claude_cli")).toBe(false);
    expect(provider).toEqual(before);
    const doc = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, draft);
    expect(doc.providers[0]).toMatchObject({ explicit_credentials_only: true, credentials: [], preset });
    expect(doc.model_routes).toEqual([]);
  });

  it.each(PRODUCTS)("locks %s even when the saved fingerprint is auto or absent", (special) => {
    for (const fingerprint of [undefined, "auto"]) {
      for (const key of ["call_flow", "callFlow"]) {
        const provider = { [key]: { special_adapter: special, ...(fingerprint ? { fingerprint_adapter: fingerprint } : {}) } };
        const before = structuredClone(provider);
        expect(readProviderRequestClientProfile(provider)).toBe("session");
        expect(writeRequestClientProfile(provider, "workbuddy")).toBe(false);
        expect(provider).toEqual(before);
      }
    }
  });

  it.each(PRODUCTS)("projects custom %s to its actual preset instead of a stale API encoder", (special, preset) => {
    const entry = { ...template("unused"), custom: true, preset: null,
      adapter: "openai_compatible", protocolProfile: "openai_compatible_generic" };
    const workflow = defaultCatalogWorkflow(entry);
    workflow.specialAdapter = special;
    workflow.callAdapter = "anthropic";
    const provider = providerDefinitionFromCatalogDraft(entry, {
      templateId: entry.id, providerId: "custom-product", providerLabel: "fixture", vendorKey: "fixture",
      vendorName: "fixture", baseUrl: entry.baseUrl, supportedModels: [], workflow, requestClientProfile: "cpa",
    });
    expect(provider.preset).toBe(preset);
    expect(provider).not.toHaveProperty("adapter");
    expect(provider).not.toHaveProperty("protocol_profile");
    expect(provider.call_flow).toEqual({ call_adapter: "auto", fingerprint_adapter: "session", special_adapter: special });
  });

  it.each(PRODUCTS)("retains the preset-owned identity of legacy %s providers", (_, preset) => {
    for (const flow of [undefined, {}, { special_adapter: "auto", fingerprint_adapter: "auto" }]) {
      const provider = { preset, request_client_profile: "codex_cli", ...(flow ? { call_flow: flow } : {}) };
      const before = structuredClone(provider);
      expect(readProviderRequestClientProfile(provider)).toBe("session");
      expect(writeRequestClientProfile(provider, "claude_cli")).toBe(false);
      expect(provider).toEqual(before);
    }
  });

  it("does not infer a session from a user-editable provider ID or the optional Basispoints preset", () => {
    for (const preset of ["xai-api", "chatgpt-codex-oauth-official-api"]) {
      const provider = { id: "grok-web", preset };
      expect(readProviderRequestClientProfile(provider)).toBe("auto");
      expect(writeRequestClientProfile(provider, "codex_cli")).toBe(true);
    }
  });
});
