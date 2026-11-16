import { useEffect, useMemo, useRef, useState } from "react";
import { createFreebuffPurchaseApi, type FreebuffPendingPurchase, type FreebuffPurchaseDecision } from "../../api/console/freebuff-purchases";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";

/** A decision belongs to one displayed snapshot. No polling, automatic approval or mutation replay. */
export function useFreebuffPurchases(props: FreebuffCatalogProps, target: FreebuffBusinessTarget) {
  const scope = useMemo(() => ({ api: props.client, providerId: props.providerId, credentialId: props.credentialId,
    expectedRevision: props.expectedRevision, token: props.managementToken, grant: props.secretGrant, disabled: props.disabled,
    origin: target.origin, client: target.client }), [props.client, props.providerId, props.credentialId, props.expectedRevision,
    props.managementToken, props.secretGrant, props.disabled, target.origin, target.client]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<{ controller: AbortController; write: boolean } | null>(null);
  type Result = { scope: typeof scope; rows: FreebuffPendingPurchase[] };
  type Choice = { result: Result; row: FreebuffPendingPurchase; decision: FreebuffPurchaseDecision };
  const [result, setResult] = useState<Result | null>(null);
  const [choice, setChoice] = useState<Choice | null>(null);
  const [feedback, setFeedback] = useState<{ scope: typeof scope; error?: string; delivered?: FreebuffPurchaseDecision } | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    setResult(null); setChoice(null); setFeedback(null); setBusy(false);
    return () => { pending.current?.controller.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.providerId && !!scope.credentialId && !!scope.expectedRevision && !!scope.token;
  const rows = result?.scope === scope ? result.rows : null;
  const selected = choice?.result === result && result?.scope === scope ? choice : null;
  const run = async (write: boolean) => {
    if (!enabled || current.current !== scope || pending.current || (write && !selected)) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    const controller = new AbortController(); pending.current = { controller, write };
    setBusy(true); setResult(null); setChoice(null); setFeedback(null);
    const api = createFreebuffPurchaseApi(scope.api, scope.token!);
    const binding = { providerId: scope.providerId, credentialId: scope.credentialId, expectedRevision: scope.expectedRevision };
    try {
      if (write) await api.decide(binding, selected!.row, selected!.decision, scope.grant, controller.signal);
      else {
        const values = await api.read(binding, scope.grant, controller.signal);
        if (!controller.signal.aborted && current.current === scope) setResult({ scope, rows: values });
      }
      if (write && !controller.signal.aborted && current.current === scope) setFeedback({ scope, delivered: selected!.decision });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setFeedback({ scope, error: write
        ? "决定交付结果未确认，请重新读取待确认购买；不会自动重试。"
        : "待确认购买读取失败，请核对账号、版本与管理权限。" });
    } finally {
      if (pending.current?.controller === controller) { pending.current = null; setBusy(false); }
    }
  };
  const prepare = (row: FreebuffPendingPurchase, decision: FreebuffPurchaseDecision) => {
    if (!enabled || pending.current || current.current !== scope || !result || !rows?.includes(row)) return;
    if (!scope.grant) { props.onRequestSecretAccess(); return; }
    setChoice({ result, row, decision });
  };
  const cancel = () => {
    const write = pending.current?.write; pending.current?.controller.abort(); pending.current = null;
    setBusy(false); setResult(null); setChoice(null);
    setFeedback(write ? { scope, error: "已停止等待，决定可能已交付；请重新读取，不会自动重试。" } : null);
  };
  return { enabled, busy, rows, selected, prepare, cancel, read: () => run(false), confirm: () => run(true),
    dismiss: () => setChoice(null), feedback: feedback?.scope === scope ? feedback : null };
}
