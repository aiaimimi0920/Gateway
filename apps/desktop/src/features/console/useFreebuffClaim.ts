import { useEffect, useMemo, useRef, useState } from "react";
import { freebuffClaimEnd, freebuffClaimJournal, type FreebuffClaimEnd, type FreebuffClaimJournal } from "./freebuffClaim";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";
const endpoint = "/v1/internal/gateway/console/freebuff-session-claim";
/** Read before each explicit end; cancellation is not rollback and never authorizes replay. */
export function useFreebuffClaim(props: FreebuffCatalogProps, target: FreebuffBusinessTarget) {
  const scope = useMemo(() => ({ api: props.client, provider: props.providerId, credential: props.credentialId, revision: props.expectedRevision,
    token: props.managementToken, grant: props.secretGrant, disabled: props.disabled, origin: target.origin, client: target.client }),
  [props.client, props.providerId, props.credentialId, props.expectedRevision, props.managementToken, props.secretGrant, props.disabled, target.origin, target.client]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<{ controller: AbortController; write: boolean } | null>(null);
  type Result = { scope: typeof scope; value: FreebuffClaimJournal };
  const [result, setResult] = useState<Result | null>(null); const [approval, setApproval] = useState<Result | null>(null);
  const [ended, setEnded] = useState<FreebuffClaimEnd | null>(null); const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  useEffect(() => { setResult(null); setApproval(null); setEnded(null); setError(null); setBusy(false);
    return () => { pending.current?.controller.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.provider && !!scope.credential && !!scope.revision && !!scope.token;
  const value = result?.scope === scope ? result.value : null;
  const approved = approval?.scope === scope && approval === result ? approval.value.claim : null;
  const matches = (value: FreebuffClaimJournal | FreebuffClaimEnd) => value.revision === scope.revision && value.providerId === scope.provider
    && value.credentialId === scope.credential && value.businessOrigin === scope.origin && value.client === scope.client;
  const run = async (write: boolean, cursor: string | null = null) => {
    if (!enabled || pending.current || current.current !== scope || (write && !approved)) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const selected = approved;
    const controller = new AbortController(); pending.current = { controller, write };
    setBusy(true); setResult(null); setApproval(null); setEnded(null); setError(null);
    try {
      const data = await scope.api.request<FreebuffClaimEnd | FreebuffClaimJournal>(write ? `${endpoint}/end` : endpoint, write ? freebuffClaimEnd : freebuffClaimJournal, {
        method: "POST", managementToken: scope.token!, secretGrant: scope.grant, signal: controller.signal,
        body: { providerId: scope.provider, credentialId: scope.credential, expectedRevision: scope.revision,
          ...(write ? { attemptId: selected!.attemptId } : { cursor }) },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      if (write) {
        const parsed = freebuffClaimEnd.safeParse(data);
        if (!parsed.success || !matches(parsed.data) || parsed.data.attemptId !== selected!.attemptId || parsed.data.modelKey !== selected!.modelKey) throw new Error("mismatched claim end");
        setEnded(parsed.data);
      } else {
        const parsed = freebuffClaimJournal.safeParse(data);
        if (!parsed.success || !matches(parsed.data) || parsed.data.cursor !== cursor) throw new Error("mismatched claim journal");
        setResult({ scope, value: parsed.data });
      }
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError(write ? "结束结果尚未确认，请重新检查本地会话与结束记录。" : "本地会话读取失败，请核对账号与管理权限。");
    } finally {
      if (pending.current?.controller === controller) { pending.current = null; setBusy(false); }
    }
  };
  const cancel = () => { const write = pending.current?.write; pending.current?.controller.abort(); pending.current = null;
    setBusy(false); setResult(null); setApproval(null); setEnded(null); if (write) setError("已停止等待，请重新检查本地会话与结束记录；远端操作不会撤销。"); };
  const prepare = () => { if (enabled && !pending.current && value?.claim) {
    if (!scope.grant) { props.onRequestSecretAccess(); return; } setApproval(result);
  } };
  return { value, approved, ended, busy, error, enabled, prepare, cancel, read: (cursor: string | null = null) => run(false, cursor),
    confirm: () => run(true), dismiss: () => setApproval(null) };
}
