// Editing policy follows the actual product, never a mutable provider ID or secret draft.
import { isRecord } from "./routeDocument";
import { isVertexExpressProvider, validVertexExpressModel } from "./vertexExpress";
import { isVertexOAuthProvider } from "./vertexOAuth";

export type ModelIdPolicy = "standard" | "bedrock" | "vertex-express" | "nous";
export const modelIdLimit = (policy: ModelIdPolicy) => policy === "standard" ? 256 : policy === "nous" ? 512 : 2048;
export function modelIdPolicy(provider: unknown, credential?: unknown): ModelIdPolicy {
  if (!isRecord(provider)) return "standard";
  if (provider.preset === "nous-portal-oauth" && provider.auth_mode === "nous_portal_oauth") return "nous";
  if (isVertexExpressProvider(provider) || isVertexOAuthProvider(provider)) return "vertex-express";
  const flow = isRecord(provider.call_flow) ? provider.call_flow : isRecord(provider.callFlow) ? provider.callFlow : {};
  if (flow.call_adapter && flow.call_adapter !== "auto") return "standard";
  // Current discovery products are not Bedrock; they replace the preset transport.
  if (isRecord(credential) && credential.discovery) return "standard";
  const adapter = provider.adapter;
  return (typeof adapter === "string" ? adapter === "bedrock_converse_compatible" : provider.preset === "bedrock-converse") ? "bedrock" : "standard";
}
export function validModelId(id: string, policy: ModelIdPolicy = "standard"): boolean {
  if (policy === "vertex-express") return validVertexExpressModel(id);
  if (policy === "nous") return id.length > 0 && id.length <= 512 && /^[\x21-\x7e]+$/.test(id) && !/[*?]/.test(id);
  const bytes = new TextEncoder().encode(id).length;
  if (!id || id.trim() !== id || bytes > modelIdLimit(policy) || /[\p{Cc}*?]/u.test(id)) return false;
  if (bytes <= 256) return true;
  // Match the native transport's URI-label grammar without trimming or recasing it.
  return /^[A-Za-z0-9_.:~/-]+$/.test(id) && (!id.includes("/") || id.startsWith("arn:"))
    && id.split("/").every((part) => part.length > 0 && part !== "." && part !== "..");
}
