import { useEffect, useMemo, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { lobsteraiCatalogResponseSchema, type LobsterAiCatalogBinding, type LobsterAiModelCatalog } from "./lobsteraiModelCatalog";
export type LobsterAiCatalogProps = {
  client: GatewayApiClient; binding: LobsterAiCatalogBinding | null; expectedRevision: string;
  managementToken: string | null; secretGrant: string | null; disabled: boolean; onRequestSecretAccess(): void;
};
/** A read has no publication side effects; changing operator, account or revision fences its result. */
export function useLobsterAiCatalog(props: LobsterAiCatalogProps) {
  const b = props.binding;
  const scope = useMemo(() => ({ client: props.client, binding: b, revision: props.expectedRevision,
    token: props.managementToken, grant: props.secretGrant, disabled: props.disabled }), [props.client, props.expectedRevision,
    props.managementToken, props.secretGrant, props.disabled, b?.providerId, b?.credentialId, b?.contextKey, b?.baseUrl]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{ scope: typeof scope; catalog: LobsterAiModelCatalog } | null>(null);
  useEffect(() => {
    setResult(null); setError(null); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.binding && !!scope.revision && !!scope.token;
  const cancel = () => { pending.current?.abort(); pending.current = null; setBusy(false); };
  const clear = () => { cancel(); setResult(null); setError(null); };
  const read = async () => {
    if (!enabled || !scope.binding || pending.current) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(null); setResult(null);
    try {
      const response = await scope.client.request("/v1/internal/gateway/console/lobsterai-model-catalog", lobsteraiCatalogResponseSchema, {
        method: "POST", managementToken: scope.token!, secretGrant: scope.grant, signal: controller.signal,
        body: { providerId: scope.binding.providerId, credentialId: scope.binding.credentialId, expectedRevision: scope.revision },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      const parsed = lobsteraiCatalogResponseSchema.safeParse(response);
      if (!parsed.success || parsed.data.revision !== scope.revision || parsed.data.providerId !== scope.binding.providerId
        || parsed.data.credentialId !== scope.binding.credentialId) {
        setError("目录响应与当前 LobsterAI 账号或配置版本不匹配。"); return;
      }
      setResult({ scope, catalog: parsed.data.catalog });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError("LobsterAI 目录读取失败，请检查账号权限后重试。");
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  return { read, cancel, clear, busy, enabled, error, catalog: result?.scope === scope ? result.catalog : null };
}
