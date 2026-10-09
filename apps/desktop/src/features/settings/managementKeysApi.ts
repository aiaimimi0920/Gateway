import { z } from "zod";
import { createGatewayApiClient } from "../../api/client";
import type { GatewayHostAdapter } from "../../platform/types";

const response = z.object({ keys: z.array(z.object({
  id: z.string(), name: z.string(), createdAt: z.string(), current: z.boolean(),
})) });
export type ManagementKey = z.infer<typeof response>["keys"][number];
export function managementKeysApi(host: GatewayHostAdapter) {
  const client = createGatewayApiClient({ host });
  const path = "/v1/internal/gateway/console/management-keys";
  return {
    list: (managementToken: string, signal?: AbortSignal) => client.request(path, response, { managementToken, signal }),
    add: (managementToken: string, name: string, token: string) => client.request(path, z.object({ success: z.literal(true) }), {
      method: "POST", managementToken, body: { action: "add", name, token },
    }),
    revoke: (managementToken: string, id: string) => client.request(path, z.object({ success: z.literal(true) }), {
      method: "POST", managementToken, body: { action: "revoke", id },
    }),
    edit: (managementToken: string, id: string, name: string, token?: string) => client.request(path, z.object({ success: z.literal(true) }), {
      method: "POST", managementToken, body: { action: "edit", id, name, token },
    }),
    reveal: (managementToken: string, id: string, signal?: AbortSignal) => client.request(path, z.object({ token: z.string().nullable() }), {
      method: "POST", managementToken, signal, body: { action: "reveal", id },
    }),
  };
}
