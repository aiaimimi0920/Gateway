import type { ProviderCatalogTemplate } from "./providerCatalog";

// These presets already exist in Rust; exposing them does not assert new-flow acceptance.
export const WEB_CATALOG_TEMPLATES: readonly ProviderCatalogTemplate[] = [
  { id: "chatgpt-web", providerId: "chatgpt-web", labelZh: "chatgpt-web", labelEn: "chatgpt-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "chatgpt-web-reverse", vendorKey: "chatgpt_platform", vendorName: "ChatGPT", baseUrl: "https://chatgpt.com", supportedModels: [] },
  { id: "gemini-web", providerId: "gemini-web", labelZh: "gemini-web", labelEn: "gemini-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "gemini-web-chat", vendorKey: "gemini_platform", vendorName: "Gemini", baseUrl: "https://gemini.google.com", supportedModels: [] },
  { id: "aistudio-web", providerId: "aistudio-web", labelZh: "aistudio-web", labelEn: "aistudio-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "aistudio-web-reverse", vendorKey: "aistudio_platform", vendorName: "AI Studio", baseUrl: "https://ai.studio", supportedModels: [] },
];
