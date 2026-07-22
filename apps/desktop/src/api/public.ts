import type { GatewayApiClient } from "./client";
import type { PublicHealth, PublicModelList, PublicReadiness } from "./contracts";
import {
  publicHealthSchema,
  publicModelListSchema,
  publicReadinessSchema,
  unknownObjectSchema,
} from "./schemas";

export type PublicGatewayApi = {
  getHealth(): Promise<PublicHealth>;
  getReadiness(): Promise<PublicReadiness>;
  getModels(apiKey?: string): Promise<PublicModelList>;
  createChatCompletion(payload: Record<string, unknown>, apiKey?: string): Promise<Record<string, unknown>>;
};

function bearerHeaders(apiKey?: string): HeadersInit | undefined {
  const token = apiKey?.trim();
  return token ? { Authorization: `Bearer ${token}` } : undefined;
}

export function createPublicGatewayApi(client: GatewayApiClient): PublicGatewayApi {
  return {
    getHealth: () => client.request("/healthz", publicHealthSchema),
    getReadiness: () => client.request("/readyz", publicReadinessSchema),
    getModels: (apiKey) =>
      client.request("/v1/models", publicModelListSchema, { headers: bearerHeaders(apiKey) }),
    createChatCompletion: (payload, apiKey) =>
      client.request("/v1/chat/completions", unknownObjectSchema, {
        method: "POST",
        headers: bearerHeaders(apiKey),
        body: payload,
      }),
  };
}
