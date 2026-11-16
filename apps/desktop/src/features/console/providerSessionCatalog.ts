import type { ProviderCatalogTemplate } from "./providerCatalog";
import type { CredentialTestPolicy } from "../../api/contracts";
import { SESSION_PRODUCTS, type SessionProduct } from "./providerSessionProducts";

// Empty model lists are intentional: a built-in protocol default is not current account entitlement evidence.
export const SESSION_CATALOG_TEMPLATES: readonly ProviderCatalogTemplate[] = Object.entries(SESSION_PRODUCTS).map(([id, product]) => ({
  id, providerId: id, labelZh: id, labelEn: id, descriptionZh: product.hint, descriptionEn: `${product.vendorName} session import; independent of API/OAuth credentials. Pending live acceptance.`,
  category: "mainstream", compatibility: "native", preset: product.preset, vendorKey: product.vendorKey,
  vendorName: product.vendorName, baseUrl: product.baseUrl, supportedModels: [],
}));

const MEDIA_PROMPTS: Partial<Record<SessionProduct, string>> = {
  "chataibot-web": "Generate an original image of a small blue ceramic cup on a plain white table.",
  "lumalabs-web": "Generate an original image of a small blue ceramic cup on a plain white table.",
  "suno-web": "Generate a short original instrumental music track with gentle piano and no vocals.",
  "udio-web": "Generate a short original instrumental music track with gentle piano and no vocals.",
};
export function applySessionCatalogTestPolicy(id: string, policy: CredentialTestPolicy) {
  const prompt = MEDIA_PROMPTS[id as SessionProduct];
  if (prompt) policy.cases = [{ id: "generation", name: "媒体生成", prompt, expectedAnswer: null, difficulty: 1, enabled: true }];
}

/** Only explicit anchors insert a product beside an existing API; unmatched brands stay at the end. */
export function webCatalogAnchor(id: string): string | undefined {
  if (id === "chatgpt-web") return "chatgpt";
  if (["antigravity-oauth", "stitch-web", "flow-web", "gemini-web", "gemini-canvas", "aistudio-web"].includes(id)) return "gemini-api";
  if (id === "grok-web") return "xai-api";
  if (id === "perplexity-web") return "perplexity-api";
  return undefined;
}
