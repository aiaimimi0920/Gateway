import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

export const probePolicySchema = z.object({
  enabled: z.boolean().default(true),
  mode: z.enum(["advisory", "restrict"]).default("advisory"),
  site_url: z.string().nullable().optional(),
  refresh_interval_secs: z.number().int().min(30).max(86400).default(300),
  max_age_secs: z.number().int().min(30).max(604800).default(900),
  minimum_success_rate: z.number().min(0).max(100).default(95),
  unknown_policy: z.enum(["allow", "block"]).default("allow"),
  monitor_groups: z.array(z.string().min(1).max(128)).max(16).default([]),
  model_map: z
    .record(z.string().min(1).max(256), z.string().min(1).max(256))
    .default({}),
  allow_private_network: z.boolean().default(false),
  adapter: z
    .enum([
      "auto",
      "new-api",
      "sub2api",
      "one-api",
      "compatible-gateway",
      "new-api-perf-summary",
      "model-status-all",
      "channel-health",
      "model-status-embed",
      "model-health-slot",
      "sub2api-v2",
      "openai-status",
      "anthropic-status",
      "deepseek-status",
      "nvidia-status",
      "gemini-status",
      "openrouter-status",
      "cohere-status",
      "groq-status",
      "poe-status",
      "perplexity-status",
      "linkup-status",
      "exa-status",
      "jina-status",
      "together-status",
      "mistral-status",
      "xai-status",
      "tavily-status",
      "you-status",
      "azure-status",
      "qwen-status",
    ])
    .default("auto"),
  auth: z
    .object({
      kind: z.enum(["none", "bearer", "api_key"]).default("none"),
      token: z.null().optional(),
      header: z.string().nullable().optional(),
    })
    .default({ kind: "none" }),
});
export const healthProfileSchema = z.object({
  id: z
    .string()
    .min(1)
    .max(64)
    .regex(/^[A-Za-z0-9_-]+$/),
  name: z.string().min(1).max(128),
  account_group_ids: z.array(z.string().min(1).max(256)).max(64).default([]),
  policy: probePolicySchema,
});
export const siteHealthConfigSchema = probePolicySchema.extend({
  profiles: z.array(healthProfileSchema).max(16).default([]),
});
export type ProbePolicy = z.infer<typeof probePolicySchema>;
export type SiteHealthConfig = z.infer<typeof siteHealthConfigSchema>;
const rates = z.record(z.string(), z.number().min(0).max(100).nullable());
export const siteSnapshotSchema = z.object({
  provider_id: z.string(),
  checked_at: z.number(),
  binding: z.string(),
  profile_id: z.string().nullable().optional(),
  selected_template: z.string().nullable(),
  models: rates,
  probes: z.array(
    z.object({
      template: z.string(),
      endpoint: z.string(),
      status: z.string(),
      observations: z
        .array(
          z.object({
            name: z.string(),
            status: z.string(),
            model: z.string().nullable(),
          }),
        )
        .default([]),
      http_status: z.number().nullable(),
      model_count: z.number().int().nonnegative(),
      models: rates,
      coverage: z
        .object({
          complete: z.boolean().nullable(),
          aggregation_lag_secs: z.number().nullable(),
          computed_at: z.string().nullable(),
        })
        .nullable()
        .optional(),
    }),
  ),
});
export const siteHealthResponseSchema = z.object({
  items: z.array(
    z.object({
      provider_id: z.string(),
      config: siteHealthConfigSchema,
      snapshot: siteSnapshotSchema.nullable(),
      profiles: z
        .array(
          z.object({
            profile_id: z.string().nullable(),
            account_group_ids: z.array(z.string()),
            snapshot: siteSnapshotSchema.nullable(),
          }),
        )
        .default([]),
    }),
  ),
});
export type SiteHealthResponse = z.infer<typeof siteHealthResponseSchema>;

export function applySiteHealth(
  document: ConsoleRouteDocument,
  id: string,
  config: SiteHealthConfig | null,
): ConsoleRouteDocument {
  const providers = document.providers.map((provider) => {
    if (!isRecord(provider) || provider.id !== id) return provider;
    const next = { ...provider };
    if (config) next.site_health = siteHealthConfigSchema.parse(config);
    else delete next.site_health;
    return next;
  });
  return { ...document, providers };
}

export function isCodexProvider(provider: unknown): boolean {
  if (!isRecord(provider)) return false;
  if (
    provider.base_url === undefined ||
    provider.base_url === null ||
    provider.base_url === ""
  ) {
    return provider.preset === "chatgpt-codex-oauth-official-api";
  }
  try {
    const url = new URL(String(provider.base_url));
    return (
      url.protocol === "https:" &&
      url.hostname === "chatgpt.com" &&
      url.pathname.replace(/\/$/, "") === "/backend-api/codex"
    );
  } catch {
    return false;
  }
}
