import { useCallback, useEffect, useRef } from "react";
import { z } from "zod";
import type { GatewayApiClient } from "../../api/client";

const responseSchema = z.object({ apiKey: z.string() });
export function useCredentialKeyReveal(client: GatewayApiClient, managementToken: string | null,
  secretGrant: string | null, revision: string | undefined, providerId?: string, credentialId?: string) {
  const generation = useRef(0);
  useEffect(() => { generation.current += 1; return () => { generation.current += 1; }; },
    [client, managementToken, secretGrant, revision, providerId, credentialId]);
  return useCallback(async (signal: AbortSignal) => {
    if (!managementToken || !secretGrant || !revision || !providerId || !credentialId) {
      throw new Error("请先确认敏感信息权限并等待账号保存。");
    }
    const started = generation.current;
    const result = await client.request("/v1/internal/gateway/console/credential-api-key", responseSchema, {
      method: "POST", managementToken, secretGrant, signal,
      body: { providerId, credentialId, expectedRevision: revision },
    });
    if (signal.aborted || started !== generation.current) throw new Error("账号或登录状态已变化，请重试。");
    return result.apiKey;
  }, [client, managementToken, secretGrant, revision, providerId, credentialId]);
}
