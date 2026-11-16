import { useEffect, useMemo, useRef, useState } from "react";
import { freebuffBusinessOrigin, freebuffClient } from "./freebuffCatalog";
import { freebuffDeviceResponse, type FreebuffDeviceState } from "./freebuffDeviceRegistration";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
const endpoint = "/v1/internal/gateway/console/freebuff-device-registration";
/** A cancelled write is not a rollback. Only an explicit local read enables the next scoped action. */
export function useFreebuffDeviceRegistration(props: FreebuffCatalogProps, origin: string, client: string, externalBusy: boolean) {
  const scope = useMemo(() => ({ api: props.client, providerId: props.providerId, credentialId: props.credentialId,
    expectedRevision: props.expectedRevision, token: props.managementToken, grant: props.secretGrant,
    disabled: props.disabled, origin, client }), [props.client, props.providerId, props.credentialId, props.expectedRevision,
    props.managementToken, props.secretGrant, props.disabled, origin, client]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<{ controller: AbortController; write: boolean } | null>(null);
  type Result = { scope: typeof scope; value: FreebuffDeviceState };
  const [result, setResult] = useState<Result | null>(null);
  const [confirmation, setConfirmation] = useState<Result | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setResult(null); setConfirmation(null); setError(null); setBusy(false);
    return () => { pending.current?.controller.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !props.disabled && !!props.expectedRevision && !!props.managementToken && !!props.providerId && !!props.credentialId
    && freebuffBusinessOrigin.safeParse(origin).success && freebuffClient.safeParse(client).success;
  const state = result?.scope === scope ? result.value : null;
  const approved = confirmation?.scope === scope && confirmation === result ? confirmation.value : null;
  const run = async (write: boolean, saved?: FreebuffDeviceState) => {
    if (!enabled || externalBusy || pending.current || current.current !== scope) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const controller = new AbortController(); pending.current = { controller, write };
    setBusy(true); setError(null); setResult(null); setConfirmation(null);
    try {
      const response = await scope.api.request(write ? endpoint : `${endpoint}/status`, freebuffDeviceResponse, {
        method: "POST", managementToken: scope.token!, secretGrant: scope.grant, signal: controller.signal,
        body: { providerId: scope.providerId, credentialId: scope.credentialId, expectedRevision: scope.expectedRevision,
          businessOrigin: scope.origin, client: scope.client,
          ...(saved?.registration === "stored" ? { replaceStoredRecordId: saved.recordId } : {}),
          ...(saved?.registration === "unconfirmed" ? { retryUnconfirmedAttemptId: saved.unconfirmedAttemptId } : {}) },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      const parsed = freebuffDeviceResponse.safeParse(response);
      if (!parsed.success || parsed.data.revision !== scope.expectedRevision || parsed.data.providerId !== scope.providerId
        || parsed.data.credentialId !== scope.credentialId || parsed.data.businessOrigin !== scope.origin || parsed.data.client !== scope.client) {
        setError(write ? "未取得注册确认，请先检查设备状态。" : "设备状态响应与当前账号或目标不匹配。"); return;
      }
      setResult({ scope, value: parsed.data });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError(write
        ? "未取得注册确认，请先检查设备状态。" : "设备状态读取失败，请检查管理权限后重试。");
    } finally {
      if (pending.current?.controller === controller) { pending.current = null; setBusy(false); }
    }
  };
  const prepare = () => {
    if (!enabled || externalBusy || pending.current || !state) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    setConfirmation(result);
  };
  const confirm = async () => { if (approved) await run(true, approved); };
  const cancel = () => {
    const write = pending.current?.write;
    pending.current?.controller.abort(); pending.current = null; setBusy(false); setConfirmation(null); setResult(null);
    if (write) setError("已停止等待，注册结果尚未确认。请先检查设备状态。");
  };
  return { state, approved, enabled, busy, error, prepare, confirm, cancel,
    read: () => run(false), dismiss: () => setConfirmation(null) };
}
