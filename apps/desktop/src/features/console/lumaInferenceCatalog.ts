// Inference uses its own sales-provisioned Key and token bill, separate from Agents credits.
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const LUMA_INFERENCE_TEMPLATE = {
  id: "lumalabs-inference-api", providerId: "lumalabs-inference-api",
  labelZh: "lumalabs-inference-api", labelEn: "lumalabs-inference-api",
  descriptionZh: "Luma LLM 独立 API Key；按 token 后付费，需销售开通。",
  descriptionEn: "Luma LLM inference Key; postpaid tokens, provisioned through sales.",
  category: "mainstream", compatibility: "openai", preset: "lumalabs-inference-api",
  protocolProfile: "lumalabs_inference", vendorKey: "lumalabs-inference", vendorName: "Luma Inference",
  baseUrl: "https://inference.lumalabs.ai/v1", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
