import { z, type ZodType } from "zod";
import { createGatewayApiClient } from "../../api/client";
import type { GatewayHostAdapter } from "../../platform/types";

const settingsSchema = z.object({
  logDirectory: z.string(), retentionDays: z.union([z.literal(0), z.literal(3), z.literal(7), z.literal(30)]),
  logLimitMb: z.union([z.literal(0), z.literal(100), z.literal(200), z.literal(500)]),
});
export type GatewayLogSettings = z.infer<typeof settingsSchema>;
const downloadSchema = z.object({
  fileName: z.enum(["gateway-diagnostics.json", "gateway-logs.log"]),
  mimeType: z.enum(["application/json;charset=utf-8", "text/plain;charset=utf-8"]),
  content: z.string().max(16 * 1024 * 1024), truncated: z.boolean(),
});
export type GatewayDownload = z.infer<typeof downloadSchema>;
export function gatewayDiagnosticsApi(host: GatewayHostAdapter) {
  const client = createGatewayApiClient({ host });
  const root = "/v1/internal/gateway/console/diagnostics";
  // A disconnected remote backend must not leave the settings navigation locked indefinitely.
  const request = async <T>(path: string, schema: ZodType<T>, managementToken: string, signal: AbortSignal,
    method = "GET", body?: unknown): Promise<T> => {
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (signal.aborted) abort();
    else signal.addEventListener("abort", abort, { once: true });
    const timer = setTimeout(abort, 30_000);
    try {
      return await client.request(`${root}/${path}`, schema, { method, managementToken, signal: controller.signal, body });
    } finally {
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
    }
  };
  return {
    load: (managementToken: string, signal: AbortSignal) => request("settings", settingsSchema, managementToken, signal),
    save: (managementToken: string, policy: Pick<GatewayLogSettings, "retentionDays" | "logLimitMb">, signal: AbortSignal) =>
      request("settings", settingsSchema, managementToken, signal, "PUT", policy),
    export: (managementToken: string, kind: "diagnostics" | "logs", signal: AbortSignal) =>
      request("export", downloadSchema, managementToken, signal, "POST", { kind }),
  };
}
