import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

export const siteHealthConfigSchema = z.object({
  mode: z.enum(["advisory", "restrict"]).default("advisory"),
  site_url: z.string().nullable().optional(),
  refresh_interval_secs: z.number().int().min(30).max(86400).default(300),
  max_age_secs: z.number().int().min(30).max(604800).default(900),
  minimum_success_rate: z.number().min(0).max(100).default(95),
  unknown_policy: z.enum(["allow", "block"]).default("allow"),
  monitor_groups: z.array(z.string().min(1).max(128)).max(16).default([]),
  model_map: z.record(z.string().min(1).max(256), z.string().min(1).max(256)).default({}),
  allow_private_network: z.boolean().default(false),
});
export type SiteHealthConfig = z.infer<typeof siteHealthConfigSchema>;
const rates = z.record(z.string(), z.number().min(0).max(100).nullable());
export const siteSnapshotSchema = z.object({
  provider_id: z.string(), checked_at: z.number(), binding: z.string(),
  selected_template: z.string().nullable(), models: rates,
  probes: z.array(z.object({ template: z.string(), endpoint: z.string(), status: z.string(),
    http_status: z.number().nullable(), model_count:z.number().int().nonnegative(), models: rates })),
});
export const siteHealthResponseSchema = z.object({ items: z.array(z.object({
  provider_id: z.string(), config: siteHealthConfigSchema, snapshot: siteSnapshotSchema.nullable(),
})) });
export type SiteHealthResponse = z.infer<typeof siteHealthResponseSchema>;

export function applySiteHealth(document: ConsoleRouteDocument, id: string, config: SiteHealthConfig | null): ConsoleRouteDocument {
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
  if (provider.base_url === undefined || provider.base_url === null || provider.base_url === "") {
    return provider.preset === "chatgpt-codex-oauth-official-api";
  }
  try { const url = new URL(String(provider.base_url));
    return url.protocol === "https:" && url.hostname === "chatgpt.com" && url.pathname.replace(/\/$/, "") === "/backend-api/codex";
  } catch { return false; }
}
