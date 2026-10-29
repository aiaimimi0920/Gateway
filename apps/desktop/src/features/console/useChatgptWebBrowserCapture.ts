import { useCallback } from "react";
import { z } from "zod";
import type { GatewayApiClient } from "../../api/client";
import type { WebBrowserCapture } from "./ChatgptWebBrowserImport";

const response = z.object({
  phase: z.enum(["login_window", "session_read", "verification_required", "verification_window"]).optional(),
  tabs: z.array(z.object({ id: z.string().max(128), title: z.string().max(128) })).max(64).optional(),
  material: z.record(z.string(), z.unknown()).optional(),
  accountName: z.string().max(254).optional(),
  catalog: z.object({ code: z.string().max(64).optional(), status: z.number().int().optional() }).optional(),
});

export function useChatgptWebBrowserCapture(client: GatewayApiClient, managementToken: string | null,
  secretGrant: string | null, expectedRevision: string | undefined, disabled: boolean): WebBrowserCapture {
  return useCallback(async (input, signal) => {
    if (disabled || !managementToken || !secretGrant || !expectedRevision || !input.providerId) throw new Error("请先保存服务商并确认敏感信息访问权限。");
    return client.request("/v1/internal/gateway/console/chatgpt-web-browser", response, {
      method: "POST", managementToken, secretGrant, signal,
      body: { ...input, expectedRevision },
    });
  }, [client, managementToken, secretGrant, expectedRevision, disabled]);
}
