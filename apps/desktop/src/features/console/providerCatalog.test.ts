import { describe, expect, it } from "vitest";
import {
  PROVIDER_CATALOG_TEMPLATES,
  providerCatalogClassification,
  providerDefinitionFromCatalogDraft,
} from "./providerCatalog";

describe("provider catalog", () => {
  it("exposes unique built-in templates for mainstream, aggregator, search, and compatible providers", () => {
    const ids = PROVIDER_CATALOG_TEMPLATES.map((template) => template.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(new Set(PROVIDER_CATALOG_TEMPLATES.map((template) => template.category))).toEqual(
      new Set(["mainstream", "aggregator", "search", "third-party-compatible"]),
    );
    expect(ids).toEqual(
      expect.arrayContaining([
        "openai",
        "anthropic",
        "gemini-api",
        "azure-openai",
        "groq-openai",
        "nvidia-openai",
        "deepseek-openai",
        "mistral-openai",
        "qwen-dashscope-openai",
        "xai-openai",
        "openrouter-openai",
        "muyuan-openai",
        "custom-api-provider",
      ]),
    );
  });

  it("classifies Muyuan as a third-party OpenAI-compatible provider without changing its stable IDs", () => {
    const template = PROVIDER_CATALOG_TEMPLATES.find(
      (candidate) => candidate.id === "muyuan-openai",
    );
    expect(template).toMatchObject({
      providerId: "muyuan-openai",
      preset: "muyuan-openai",
      vendorKey: "muyuan",
      vendorName: "Muyuan",
      category: "third-party-compatible",
      compatibility: "openai",
    });
    expect(providerCatalogClassification("muyuan-openai", "muyuan-openai")).toEqual({
      category: "third-party-compatible",
      compatibility: "openai",
    });
  });

  it("builds a custom compatible provider with the generic protocol profile instead of the official OpenAI preset", () => {
    const template = PROVIDER_CATALOG_TEMPLATES.find(
      (candidate) => candidate.id === "custom-api-provider",
    );
    expect(template).toBeDefined();
    const provider = providerDefinitionFromCatalogDraft(template!, {
      templateId: "custom-api-provider",
      providerId: "partner-gateway",
      providerLabel: "Partner Gateway",
      vendorKey: "partner",
      vendorName: "Partner",
      baseUrl: "https://partner.example.test/v1",
      supportedModels: ["partner-chat"],
      credentialId: "partner-account-1",
      accountName: "Partner Account 1",
      apiKey: "not-persisted-in-provider-definition",
    });

    expect(provider).toEqual({
      id: "partner-gateway",
      label: "Partner Gateway",
      vendor_key: "partner",
      vendor_name: "Partner",
      base_url: "https://partner.example.test/v1",
      supported_models: ["partner-chat"],
      adapter: "openai_compatible",
      protocol_profile: "openai_compatible_generic",
    });
    expect(provider).not.toHaveProperty("api_key");
    expect(
      providerCatalogClassification(
        "partner-gateway",
        null,
        "openai_compatible",
        "openai_compatible_generic",
      ),
    ).toEqual({
      category: "third-party-compatible",
      compatibility: "auto",
    });
  });
});
