import { useEffect, useMemo, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { nousCatalogResponseSchema, type NousCatalogBinding, type NousModelCatalog } from "./nousModelCatalog";
export type NousCatalogProps = {
  client: GatewayApiClient; binding: NousCatalogBinding | null; expectedRevision: string;
  managementToken: string | null; secretGrant: string | null; disabled: boolean; onRequestSecretAccess(): void;
};
export function useNousCatalog(props: NousCatalogProps) {
  const b = props.binding;
  const scope = useMemo(() => ({ client: props.client, binding: b, revision: props.expectedRevision,
    token: props.managementToken, grant: props.secretGrant, disabled: props.disabled }), [props.client, props.expectedRevision,
    props.managementToken, props.secretGrant, props.disabled, b?.providerId, b?.credentialId, b?.context, b?.baseUrl]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(false);
  const [result, setResult] = useState<{ scope: typeof scope; catalog: NousModelCatalog } | null>(null);
  useEffect(() => {
    setResult(null); setError(false); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.binding && !!scope.revision && !!scope.token;
  const cancel = () => { pending.current?.abort(); pending.current = null; setBusy(false); };
  const clear = () => { cancel(); setResult(null); setError(false); };
  const read = async () => {
    if (!enabled || !scope.binding || pending.current) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(false); setResult(null);
    try {
      const response = await scope.client.request("/v1/internal/gateway/console/nous-model-catalog", nousCatalogResponseSchema, {
        method: "POST", managementToken: scope.token!, secretGrant: scope.grant, signal: controller.signal,
        body: { providerId: scope.binding.providerId, credentialId: scope.binding.credentialId, expectedRevision: scope.revision },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      const parsed = nousCatalogResponseSchema.safeParse(response);
      if (!parsed.success || parsed.data.revision !== scope.revision || parsed.data.providerId !== scope.binding.providerId
        || parsed.data.credentialId !== scope.binding.credentialId) { setError(true); return; }
      setResult({ scope, catalog: parsed.data.catalog });
    } catch { if (!controller.signal.aborted && current.current === scope) setError(true); }
    finally { if (pending.current === controller) { pending.current = null; setBusy(false); } }
  };
  return { read, cancel, clear, busy, enabled, error, catalog: result?.scope === scope ? result.catalog : null };
}
