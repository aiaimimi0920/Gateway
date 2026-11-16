import { useEffect, useMemo, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { AuthAction, AuthBinding, AuthSession, ManagedAuthApi } from "../../api/console/managed-auth";

// Receipts contain no callback or authentication materials. The console owns a separate Map per product/identity.
export type OAuthOperations<S extends AuthSession> = Map<string, { session: S; uncertain: boolean; rejected?: boolean }>;
export type ManagedOAuthProps<S extends AuthSession> = {
  client: GatewayApiClient; managementToken: string | null; secretGrant: string | null;
  providerId: string; expectedRevision: string; credentialId?: string; disabled: boolean;
  operations: OAuthOperations<S>;
  onRequestSecretAccess(): void | Promise<void>; onSaved(): void;
};
type Policy = { polling(status: string): boolean; rejected: readonly string[]; devicePolling?: boolean; unavailable?(error: unknown): boolean };
type Command<I> = { action: "create"; input: I } | { action: AuthAction; callbackUrl?: string };

// Generation owns API identity; revision changes stop mutations but never abandon an accepted save.
export function useManagedOAuth<I extends AuthBinding, S extends AuthSession>(
  props: ManagedOAuthProps<S>, createApi: (client: GatewayApiClient, token: string) => ManagedAuthApi<I, S>, policy: Policy,
) {
  const { client, managementToken, secretGrant, providerId, expectedRevision, credentialId, disabled, operations } = props;
  const operationKey = JSON.stringify([providerId, credentialId ?? null]);
  const api = useMemo(() => createApi(client, managementToken ?? ""), [client, managementToken, createApi]);
  const { polling } = policy;
  const [session, setSession] = useState<S | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [uncertain, setUncertain] = useState(false);
  const [pollStopped, setPollStopped] = useState(false);
  const [pollVersion, setPollVersion] = useState(0);
  const [pending, setPending] = useState<{ command: Command<I>; previous: string | null; epoch: number } | null>(null);
  const generation = useRef(0);
  const sessionRef = useRef<S | null>(null);
  const busyRef = useRef(false);
  const saveAttempted = useRef(false);
  const ownerGrant = useRef(secretGrant);
  const saved = useRef<string | null>(null);
  const mutationVersion = useRef(0);
  const callbacks = useRef(props);
  callbacks.current = props;
  const update = (next: S | null, record = true) => {
    sessionRef.current = next; setSession(next);
    if (record && next && saveAttempted.current) operations.set(operationKey, { session: next, uncertain });
    if (next?.status === "succeeded" && saved.current !== next.id) {
      operations.delete(operationKey);
      saved.current = next.id; setError(null); setUncertain(false); callbacks.current.onSaved();
    }
  };
  useEffect(() => { ownerGrant.current = secretGrant; }, [secretGrant]);
  useEffect(() => {
    generation.current += 1;
    setBusy(null); busyRef.current = false; setPending(null); setError(null);
    setUncertain(false); saveAttempted.current = false; saved.current = null; setPollStopped(false);
    const retained = operations.get(operationKey);
    if (retained) {
      saveAttempted.current = !retained.rejected; setUncertain(retained.uncertain);
      if (retained.rejected) { operations.delete(operationKey); setError("保存前校验被拒绝；未接纳保存，请核对后明确重试。"); }
      update(retained.session, false);
    } else update(null);
    return () => {
      generation.current += 1;
      const previous = sessionRef.current;
      const grant = ownerGrant.current;
      if (previous && grant && !saveAttempted.current && polling(previous.status)) {
        void api.act(previous.id, "cancel", { providerId, expectedRevision: previous.expectedRevision, credentialId }, grant).catch(() => {});
      }
    };
  }, [api, client, managementToken, providerId, credentialId, operations]);

  useEffect(() => {
    if (!session || !polling(session.status) || busy) return;
    const abort = new AbortController();
    const epoch = generation.current;
    let timer: ReturnType<typeof setTimeout>;
    let failures = 0;
    const poll = async () => {
      const version = mutationVersion.current;
      try {
        const result = await api.get(session.id, { providerId, expectedRevision: session.expectedRevision, credentialId }, abort.signal);
        if (abort.signal.aborted || generation.current !== epoch || mutationVersion.current !== version) return;
        const retained = operations.get(operationKey);
        if (retained?.session.id === result.id && retained.rejected) {
          operations.delete(operationKey); saveAttempted.current = false; setUncertain(false);
          setError("保存前校验被拒绝；未接纳保存，请核对后明确重试。");
        }
        failures = 0; setPollStopped(false); update(result);
        if (!polling(result.status)) return;
      } catch (cause) {
        if (abort.signal.aborted || generation.current !== epoch || mutationVersion.current !== version) return;
        if (policy.unavailable?.(cause)) {
          if (saveAttempted.current) {
            // A reclaimed receipt is not evidence of a failed commit. Preserve the duplicate-save fence.
            setUncertain(true); setPollStopped(true);
            operations.set(operationKey, { session: sessionRef.current!, uncertain: true });
            setError("保存回执已不可用；请核对当前配置与事务恢复结果，不要重复提交。");
          } else {
            update(null); setPending(null); setError("授权会话已不可用，请重新授权。");
          }
          return;
        }
        failures += 1;
        if (failures >= 5) { setError("状态读取已暂停，请检查连接后继续检查；不会重新提交保存。"); setPollStopped(true); return; }
      }
      timer = setTimeout(poll, 1500);
    };
    timer = setTimeout(poll, 1500);
    return () => { abort.abort(); clearTimeout(timer); };
  }, [api, providerId, credentialId, session?.id, session?.status, busy, pollVersion]);

  const requestAccess = async (command: Command<I>) => {
    const epoch = generation.current;
    setPending({ command, previous: secretGrant, epoch }); setError(null);
    try { await callbacks.current.onRequestSecretAccess(); }
    catch {
      if (generation.current === epoch) { setPending(null); setError("敏感信息访问权限恢复失败，请重试原操作。"); }
    }
  };
  const run = async (command: Command<I>, renewed = false) => {
    const previous = sessionRef.current;
    if (busyRef.current || !managementToken) return;
    if (command.action !== "cancel" && (disabled || !expectedRevision || previous && previous.expectedRevision !== expectedRevision)) return;
    if (command.action === "create" && command.input.expectedRevision !== expectedRevision) return;
    if (command.action === "create" && saveAttempted.current) return;
    if (command.action === "create" ? previous && polling(previous.status) : !previous) return;
    if (command.action === "import" && (saveAttempted.current || previous?.status !== "ready")) return;
    if (command.action === "complete" && (policy.devicePolling || previous?.status !== "waiting_user")) return;
    if (command.action === "poll" && (!policy.devicePolling || previous?.status !== "waiting_user")) return;
    if (!secretGrant) { await requestAccess(command); return; }
    setPending(null); setError(null); busyRef.current = true; setBusy(command.action); mutationVersion.current += 1;
    const epoch = generation.current;
    const binding = { providerId, expectedRevision: previous?.expectedRevision ?? expectedRevision, credentialId };
    if (command.action === "import") {
      if (!operations.has(operationKey) && operations.size >= 16) { setError("未确认保存记录已达上限；请先核对现有操作。"); busyRef.current = false; setBusy(null); return; }
      saveAttempted.current = true;
      operations.set(operationKey, { session: previous!, uncertain: true });
    }
    try {
      const result = command.action === "create" ? await api.create(command.input, secretGrant)
        : await api.act(previous!.id, command.action, binding, secretGrant, command.callbackUrl);
      if (generation.current !== epoch) {
        if (command.action === "create") void api.act(result.id, "cancel", command.input, secretGrant).catch(() => {});
        const retained = operations.get(operationKey);
        if (command.action === "import" && retained?.session.id === previous!.id
          && !["succeeded", "save_unconfirmed"].includes(retained.session.status)) {
          operations.set(operationKey, { session: result, uncertain: false });
        }
        return;
      }
      update(result);
    } catch (cause) {
      const rejected = cause instanceof GatewayApiError && policy.rejected.includes(cause.code ?? "");
      if (command.action === "import" && rejected && operations.get(operationKey)?.session.id === previous!.id) {
        if (generation.current !== epoch) operations.set(operationKey, { session: previous!, uncertain: false, rejected: true });
        else operations.delete(operationKey);
      }
      if (generation.current !== epoch) return;
      if (cause instanceof GatewayApiError && cause.code === "console_secret_access_required") {
        if (command.action === "import") saveAttempted.current = false;
        if (!renewed) await requestAccess(command);
        else setError("敏感信息访问权限仍不可用，请重试原操作。");
      } else {
        if (command.action === "import") {
          saveAttempted.current = !rejected; setUncertain(!rejected);
        }
        setError(command.action === "import" ? rejected ? "保存前校验被拒绝；未接纳保存。请核对原配置与权限后明确重试，不会自动更换版本。" : "保存响应未确认；仅继续检查状态，不会自动重新提交。"
          : "登录操作未完成，请检查会话状态、已保存配置与授权参数。不会自动重试。");
      }
    } finally {
      if (generation.current === epoch) { busyRef.current = false; setBusy(null); }
    }
  };
  useEffect(() => {
    // Resume only the captured explicit action after a new grant, within the same owner and revision.
    const pendingRevision = pending?.command.action === "create" ? pending.command.input.expectedRevision : session?.expectedRevision;
    if (pending && pending.command.action !== "cancel" && pendingRevision !== expectedRevision) {
      setPending(null); setError("配置版本已变化，原待执行操作已撤销；不会自动更换版本。"); return;
    }
    if (pending && pending.epoch === generation.current && secretGrant && secretGrant !== pending.previous && !busy
      && (pending.command.action === "cancel" || !disabled && (!session || session.expectedRevision === expectedRevision)
        && (pending.command.action !== "create" || pending.command.input.expectedRevision === expectedRevision))) {
      setPending(null); void run(pending.command, true);
    }
  }, [pending, secretGrant, disabled, busy, expectedRevision]);
  return { session, busy, error, uncertain, pending: Boolean(pending), pollStopped,
    stale: Boolean(session && session.expectedRevision !== expectedRevision),
    active: Boolean(session && polling(session.status)), run,
    resumePolling: () => { setPollStopped(false); setError(null); setPollVersion((value) => value + 1); } };
}
