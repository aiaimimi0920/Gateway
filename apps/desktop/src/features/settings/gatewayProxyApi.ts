import { z } from "zod";
import { createGatewayApiClient } from "../../api/client";
import type { GatewayHostAdapter } from "../../platform/types";

const mode = z.enum(["auto", "off", "custom"]);
const policySchema = z.object({ mode, url: z.string().max(2048) });
const snapshotSchema = policySchema.extend({ effectiveMode: mode, restartRequired: z.boolean() });
export type ProxyPolicy = z.infer<typeof policySchema>;
export type ProxySnapshot = z.infer<typeof snapshotSchema>;
export function gatewayProxyApi(host: GatewayHostAdapter) {
  const client = createGatewayApiClient({ host });
  const root = "/v1/internal/gateway/console/proxy";
  return {
    load: (managementToken: string, signal: AbortSignal) => client.request(`${root}/settings`, snapshotSchema, { managementToken, signal }),
    save: (managementToken: string, body: ProxyPolicy, signal: AbortSignal) => client.request(`${root}/settings`, snapshotSchema, { method: "PUT", managementToken, signal, body }),
    test: (managementToken: string, body: ProxyPolicy, signal: AbortSignal) => client.request(`${root}/test`, z.object({ ok: z.literal(true), latencyMs: z.number().nonnegative() }), { method: "POST", managementToken, signal, body }),
  };
}
