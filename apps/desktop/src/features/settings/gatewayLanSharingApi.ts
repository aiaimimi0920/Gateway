import { z } from "zod";
import { createGatewayApiClient } from "../../api/client";
import type { GatewayHostAdapter } from "../../platform/types";

const settingsSchema = z.object({ enabled: z.boolean() });
export function gatewayLanSharingApi(host: GatewayHostAdapter) {
  const client = createGatewayApiClient({ host });
  const path = "/v1/internal/gateway/console/lan-sharing";
  return {
    load: (managementToken: string, signal: AbortSignal) => client.request(path, settingsSchema, { managementToken, signal }),
    save: (managementToken: string, enabled: boolean, signal: AbortSignal) => client.request(path, settingsSchema, { method: "PUT", managementToken, signal, body: { enabled } }),
  };
}
