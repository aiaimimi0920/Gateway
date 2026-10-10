import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";

const discoveryProtocol = z.enum(["chat_completions", "responses", "messages", "gemini_generate_content",
  "gemini_interactions", "ollama_chat", "ollama_generate", "cohere_chat", "bedrock_converse", "completions", "dashscope_text", "dashscope_multimodal"]);
export const accountDiscoverySchema = z.object({
  source_url: z.string(), api_base: z.string(),
  protocol: discoveryProtocol,
  models: z.array(z.string()).min(1).max(2048), verified_models: z.array(z.string()),
  checked_at: z.string(), binding: z.string(),
  protocols: z.array(z.object({ protocol: discoveryProtocol,
    api_base: z.string(), verified_models: z.array(z.string()).min(1), failed_models: z.array(z.string()).default([]),
  })).max(12).optional(),
  probes: z.array(z.object({ protocol: discoveryProtocol, routing_ready: z.boolean().default(false),
    status: z.string(), attempts: z.array(z.object({ endpoint: z.string(), model: z.string().nullable(),
      status: z.string(), http_status: z.number().optional(), })).max(24),
  })).max(12).optional(),
});
export type AccountDiscovery = z.infer<typeof accountDiscoverySchema>;
export type DiscoveryInput = { baseUrl?: string; apiKey?: string; credentialId?: string };
export type DiscoverAccount = (input: DiscoveryInput, signal?: AbortSignal) => Promise<AccountDiscovery>;
export const discoveryResponseSchema = z.object({ discovery: accountDiscoverySchema, revision: z.string() });

export function supportsAccountDiscovery(provider: Record<string, unknown>): boolean {
  return !provider.preset && ["openai_compatible", "anthropic_compatible", "dashscope_compatible", "dashscope_multimodal_compatible"].includes(String(provider.adapter));
}

/** Update one account, preserving other accounts and existing route policies. */
export function applyAccountDiscovery(document: ConsoleRouteDocument, providerId: string,
  credentialId: string, discovery: AccountDiscovery): ConsoleRouteDocument {
  const next = structuredClone(document);
  const provider = next.providers.find((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === providerId) as Record<string, unknown> | undefined;
  const credentials = provider?.credentials as Record<string, unknown>[] | undefined;
  const credential = credentials?.find((c) => c.id === credentialId);
  if (!provider || !credential) throw new Error("账号已不存在，请刷新后重试。");
  delete credential.discovery_job;
  credential.discovery = discovery;
  credential.supported_models = [...discovery.models];
  const inherited = credentials!.some((c) => c !== credential && !c.discovery && !Array.isArray(c.supported_models));
  const previous = inherited && Array.isArray(provider.supported_models) ? provider.supported_models as string[] : [];
  provider.supported_models = [...new Set([...previous, ...credentials!.flatMap((c) => Array.isArray(c.supported_models) ? c.supported_models as string[] : [])])];
  for (const model of discovery.models) {
    const route = next.model_routes.find((r) => !!r && typeof r === "object" && (r as Record<string, unknown>).pattern === model) as Record<string, unknown> | undefined;
    if (!route) next.model_routes.push({ pattern: model, provider_ids: [providerId], priority: 10, enabled: true });
    else if (Array.isArray(route.provider_ids) && !route.provider_ids.includes(providerId)) route.provider_ids.push(providerId);
  }
  return next;
}
