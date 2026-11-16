import { useEffect, useMemo, useRef, useState } from "react";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";
import { freebuffInspectionResponse, type FreebuffInspection } from "./freebuffInspection";
export function useFreebuffInspection(props: FreebuffCatalogProps, target: FreebuffBusinessTarget | undefined, modelKey: string) {
  const scope = useMemo(() => ({ api: props.client, provider: props.providerId, credential: props.credentialId,
    revision: props.expectedRevision, token: props.managementToken, grant: props.secretGrant, disabled: props.disabled,
    origin: target?.origin, client: target?.client, modelKey }), [props.client, props.providerId, props.credentialId,
    props.expectedRevision, props.managementToken, props.secretGrant, props.disabled, target?.origin, target?.client, modelKey]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ scope: typeof scope; value: FreebuffInspection } | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    setResult(null); setError(false); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.origin && !!scope.client && !!scope.modelKey && !!scope.revision && !!scope.token;
  const read = async () => {
    if (!enabled || pending.current) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(false); setResult(null);
    try {
      const response = await scope.api.request("/v1/internal/gateway/console/freebuff-session-inspection", freebuffInspectionResponse, {
        method: "POST", managementToken: scope.token!, secretGrant: scope.grant,
        body: { providerId: scope.provider, credentialId: scope.credential, expectedRevision: scope.revision, modelKey: scope.modelKey }, signal: controller.signal,
      });
      if (controller.signal.aborted || current.current !== scope) return;
      const parsed = freebuffInspectionResponse.safeParse(response);
      if (!parsed.success || parsed.data.revision !== scope.revision || parsed.data.providerId !== scope.provider
        || parsed.data.credentialId !== scope.credential || parsed.data.businessOrigin !== scope.origin
        || parsed.data.client !== scope.client || parsed.data.modelKey !== scope.modelKey) { setError(true); return; }
      setResult({ scope, value: parsed.data });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError(true);
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  const cancel = () => { pending.current?.abort(); pending.current = null; setBusy(false); };
  return { read, cancel, busy, enabled, error, value: result?.scope === scope ? result.value : null };
}
