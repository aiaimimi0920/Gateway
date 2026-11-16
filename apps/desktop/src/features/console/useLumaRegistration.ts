// Only explicit prepare/start create work. After an accepted or ambiguous start, recovery is GET-only.
import { useEffect, useMemo, useRef, useState } from "react";
import { lumaRegistrationSchema, type LumaRegistrationView } from "../../api/console/luma-registration";
import type { LumaOAuthProps } from "./useLumaOAuth";
type Receipt = { view: LumaRegistrationView; submitted: boolean };
// Lifetime is the existing console-owned OAuth operations Map, isolated by connection/management identity.
const receipts = new WeakMap<LumaOAuthProps["operations"], Map<string, Receipt>>();
const PATH = "/v1/internal/gateway/console/luma-client-registrations";

export function useLumaRegistration(props: LumaOAuthProps & { offlineAccess?: boolean }) {
  const store = useMemo(() => {
    let store = receipts.get(props.operations);
    if (!store) { store = new Map(); receipts.set(props.operations, store); }
    return store;
  }, [props.operations]);
  const scope = useMemo(() => ({ client: props.client, token: props.managementToken, provider: props.providerId, store,
    offline: props.offlineAccess ?? false, key: props.offlineAccess ? JSON.stringify([props.providerId, "offline"]) : props.providerId }),
    [props.client, props.managementToken, props.providerId, props.offlineAccess, store]);
  const current = useRef(scope); current.current = scope;
  const latest = useRef(props); latest.current = props;
  const request = useRef<AbortController | null>(null);
  const [receipt, setReceipt] = useState<Receipt | null>(() => store.get(scope.key) ?? null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(false);
  useEffect(() => {
    setReceipt(store.get(scope.key) ?? null); setBusy(false); setError(false);
    return () => { request.current?.abort(); request.current = null; };
  }, [scope, store]);
  const update = (next: Receipt) => { store.set(scope.key, next); if (current.current === scope) setReceipt(next); };
  const run = async (action: "prepare" | "start" | "read") => {
    if (request.current || !scope.token) return;
    const p = latest.current, retained = store.get(scope.key);
    if (action !== "read" && (p.disabled || !p.expectedRevision)) return;
    if (action !== "read" && !p.secretGrant) {
      try { await p.onRequestSecretAccess(); } catch { if (current.current === scope) setError(true); }
      return;
    }
    if (action === "prepare" && retained && (retained.submitted || !["prepared", "expired", "cancelled"].includes(retained.view.status))) return;
    if (action !== "prepare" && !retained) return;
    if (action === "start" && (retained!.submitted || retained!.view.status !== "prepared"
      || retained!.view.expectedRevision !== p.expectedRevision)) return;
    const controller = new AbortController(); request.current = controller;
    setBusy(true); setError(false);
    // Retain before POST, so close/response loss cannot expose another start button.
    if (action === "start") update({ ...retained!, submitted: true });
    const path = action === "prepare" ? PATH : `${PATH}/${encodeURIComponent(retained!.view.id)}${action === "start" ? "/start" : ""}`;
    try {
      const { registration: view } = await scope.client.request(path, lumaRegistrationSchema, {
        method: action === "read" ? "GET" : "POST", cache: "no-store", managementToken: scope.token,
        secretGrant: action === "read" ? undefined : p.secretGrant, signal: controller.signal,
        body: action === "read" ? undefined : action === "prepare" ? { providerId: scope.provider, expectedRevision: p.expectedRevision, ...(scope.offline ? { offlineAccess: true } : {}) }
          : { expectedRevision: retained!.view.expectedRevision, confirmRegistration: true },
      });
      if (view.providerId !== scope.provider || view.offlineAccess !== scope.offline || (action !== "prepare" && retained && (view.id !== retained.view.id
        || view.expectedRevision !== retained.view.expectedRevision || view.redirectUri !== retained.view.redirectUri))) throw new Error("binding");
      if (controller.signal.aborted || current.current !== scope) return;
      // Expired/cancelled are authoritative never-started states, safe to prepare again.
      update({ view, submitted: !["expired", "cancelled"].includes(view.status)
        && ((action !== "prepare" && retained?.submitted === true) || action === "start" || view.status !== "prepared") });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError(true);
    } finally {
      if (request.current === controller) { request.current = null; setBusy(false); }
    }
  };
  // Reads are bounded/manual. A lost or expired receipt never starts a new registration.
  return { receipt: receipt && store.get(scope.key) === receipt ? receipt : store.get(scope.key) ?? null,
    busy, error, run };
}
