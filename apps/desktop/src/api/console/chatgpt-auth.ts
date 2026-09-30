import { z } from "zod";
import type { GatewayApiClient } from "../client";
const sessionSchema = z.object({ session: z.object({
  id: z.string(), status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "failed", "cancelled"]),
  message: z.string(), authorizationUrl: z.string().url(), credentialId: z.string().nullable(),
}) });
export type ChatgptAuthSession = z.infer<typeof sessionSchema>["session"];
const root = "/v1/internal/gateway/console/chatgpt-auth-sessions";
export function createChatgptAuthApi(client: GatewayApiClient, managementToken: string) {
  return {
    create: (providerId: string, group: string, secretGrant: string) => client.request(root, sessionSchema,
      { method: "POST", managementToken, secretGrant, body: { providerId, group } }),
    get: (id: string, signal?: AbortSignal) => client.request(`${root}/${encodeURIComponent(id)}`, sessionSchema, { managementToken, signal }),
    act: (id: string, action: "cancel" | "complete" | "import" | "open-browser", secretGrant?: string, callbackUrl?: string) =>
      client.request(`${root}/${encodeURIComponent(id)}/${action}`, sessionSchema,
        { method: "POST", managementToken, secretGrant, body: { callbackUrl } }),
  };
}
