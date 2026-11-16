import { useEffect, useMemo, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { freebuffBusinessOrigin, freebuffCatalogResponse, freebuffClient, type FreebuffCatalog } from "./freebuffCatalog";
export type FreebuffCatalogProps = {
  client: GatewayApiClient; providerId: string; credentialId: string; expectedRevision: string;
  managementToken: string | null; secretGrant: string | null; disabled: boolean;
  onRequestSecretAccess(): void;
};
export function useFreebuffCatalog(props: FreebuffCatalogProps, origin: string, client: string) {
  const scope = useMemo(() => ({ api: props.client, providerId: props.providerId, credentialId: props.credentialId,
    expectedRevision: props.expectedRevision, token: props.managementToken, grant: props.secretGrant,
    disabled: props.disabled, origin, client }), [props.client, props.providerId, props.credentialId, props.expectedRevision,
    props.managementToken, props.secretGrant, props.disabled, origin, client]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ scope: typeof scope; value: FreebuffCatalog } | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setResult(null); setError(null); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !props.disabled && !!props.expectedRevision && !!props.managementToken && !!props.providerId && !!props.credentialId
    && freebuffBusinessOrigin.safeParse(origin).success && freebuffClient.safeParse(client).success;
  const read = async () => {
    if (!enabled || pending.current) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(null); setResult(null);
    try {
      const response = await scope.api.request("/v1/internal/gateway/console/freebuff-model-catalog", freebuffCatalogResponse, {
        method: "POST", managementToken: scope.token!, secretGrant: scope.grant,
        body: { providerId: scope.providerId, credentialId: scope.credentialId, expectedRevision: scope.expectedRevision,
          businessOrigin: scope.origin, client: scope.client }, signal: controller.signal,
      });
      if (controller.signal.aborted || current.current !== scope) return;
      const parsed = freebuffCatalogResponse.safeParse(response);
      if (!parsed.success || parsed.data.revision !== scope.expectedRevision || parsed.data.providerId !== scope.providerId
        || parsed.data.credentialId !== scope.credentialId || parsed.data.businessOrigin !== scope.origin || parsed.data.client !== scope.client) {
        setError("目录响应与当前账号或目标不匹配。"); return;
      }
      setResult({ scope, value: parsed.data });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError("目录读取失败，请检查账号权限与业务站点后重试。");
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  const cancel = () => { pending.current?.abort(); pending.current = null; setBusy(false); };
  const clear = () => { cancel(); setResult(null); setError(null); };
  return { read, cancel, clear, enabled, busy, error, catalog: result?.scope === scope ? result.value : null };
}
