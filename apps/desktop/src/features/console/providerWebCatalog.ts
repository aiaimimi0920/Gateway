import type { ProviderCatalogTemplate } from "./providerCatalog";

// These presets already exist in Rust; exposing them does not assert new-flow acceptance.
export const WEB_CATALOG_TEMPLATES: readonly ProviderCatalogTemplate[] = [
  { id: "antigravity-oauth", providerId: "antigravity-oauth", labelZh: "antigravity-oauth", labelEn: "antigravity-oauth",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "antigravity-oauth", vendorKey: "antigravity_platform", vendorName: "Antigravity", baseUrl: "https://daily-cloudcode-pa.googleapis.com/v1internal", supportedModels: [] },
  { id: "stitch-web", providerId: "stitch-web", labelZh: "stitch-web", labelEn: "stitch-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "stitch-web", vendorKey: "stitch_platform", vendorName: "Google Stitch", baseUrl: "https://stitch.withgoogle.com", supportedModels: [] },
  { id: "flow-web", providerId: "flow-web", labelZh: "flow-web", labelEn: "flow-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "flow-web", vendorKey: "flow_platform", vendorName: "Google Flow", baseUrl: "https://flow.google.com", supportedModels: [] },
  { id: "chatgpt-web", providerId: "chatgpt-web", labelZh: "chatgpt-web", labelEn: "chatgpt-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "chatgpt-web-reverse", vendorKey: "chatgpt_platform", vendorName: "ChatGPT", baseUrl: "https://chatgpt.com", supportedModels: [] },
  { id: "gemini-web", providerId: "gemini-web", labelZh: "gemini-web", labelEn: "gemini-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "gemini-web-chat", vendorKey: "gemini_platform", vendorName: "Gemini", baseUrl: "https://gemini.google.com", supportedModels: [] },
  { id: "gemini-canvas", providerId: "gemini-canvas", labelZh: "gemini-canvas", labelEn: "gemini-canvas",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "gemini-canvas-browser-relay", vendorKey: "gemini_platform", vendorName: "Gemini", baseUrl: "https://gemini.google.com", supportedModels: [] },
  { id: "aistudio-web", providerId: "aistudio-web", labelZh: "aistudio-web", labelEn: "aistudio-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "aistudio-web-reverse", vendorKey: "aistudio_platform", vendorName: "AI Studio", baseUrl: "https://aistudio.google.com", supportedModels: [] },
  { id: "flowmusic-web", providerId: "flowmusic-web", labelZh: "flowmusic-web", labelEn: "flowmusic-web",
    descriptionZh: "", descriptionEn: "", category: "mainstream", compatibility: "native",
    preset: "producer", vendorKey: "producer_platform", vendorName: "FlowMusic", baseUrl: "https://www.flowmusic.app", supportedModels: ["producer:standard"] },
];
