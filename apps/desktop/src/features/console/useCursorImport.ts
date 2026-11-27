import { useEffect, useMemo, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import { createCursorAuthApi, type CursorInput, type CursorSession } from "../../api/console/cursor-auth";
type Operation = { input: CursorInput; session: CursorSession | null; notified?: boolean };
export type CursorOperations = Map<string, Operation>;
export type CursorImportProps = {
  client: GatewayApiClient; managementToken: string | null; secretGrant: string | null;
  providerId: string; expectedRevision: string; credentialId?: string; disabled: boolean;
  operations: CursorOperations; onSaved(): void; onRequestSecretAccess(): void | Promise<void>;
  onActivityChange?(active: boolean): void;
  savedAccount?: { name: string; expiresAt: string | null; hasToken: boolean };
};
const active = (s: CursorSession | null) => !s || ["waiting_user", "verifying", "awaiting_permission", "importing"].includes(s.status);
export function useCursorImport(props: CursorImportProps) {
  const { providerId, credentialId, managementToken, client, operations } = props;
  const key = JSON.stringify([providerId, credentialId ?? null]);
  const api = useMemo(() => createCursorAuthApi(client, managementToken ?? ""), [client, managementToken]);
  const [operation, setOperation] = useState<Operation | null>(() => operations.get(key) ?? null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [unavailable, setUnavailable] = useState(false);
  const [renewRequested, setRenewRequested] = useState<{ before: string | null; generation: number } | null>(null);
  const epoch = useRef(0), busyRef = useRef(false);
  const renewing = useRef(false);
  const callbacks = useRef(props); callbacks.current = props;
  const publish = (op: Operation) => {
    if (operations.get(key)?.input.attemptId !== op.input.attemptId) return;
    const notify = op.session?.status === "succeeded" && !operations.get(key)?.notified;
    const next = { ...op, notified: operations.get(key)?.notified || notify };
    operations.set(key, next); setOperation(next);
    if (notify) callbacks.current.onSaved();
  };
  useEffect(() => {
    epoch.current++; busyRef.current = false; setBusy(false); setError(null); setPaused(false); setUnavailable(false); setRenewRequested(null);
    const cached = operations.get(key);
    if (cached) publish(cached); else setOperation(null);
    // Authorization explicitly includes automatic saving; closing the panel does not cancel it.
    return () => { epoch.current++; };
  }, [api, key, operations]);
  useEffect(() => {
    if (!operation || !active(operation.session) || paused || busy) return;
    const abort = new AbortController(), generation = epoch.current;
    let timer: ReturnType<typeof setTimeout>, failures = 0;
    const poll = async () => {
      try {
        const session = await api.get(operation.input, abort.signal);
        if (abort.signal.aborted || epoch.current !== generation) return;
        failures = 0; setError(null); setUnavailable(false); publish({ ...operation, session });
        if (!active(session)) return;
      } catch (cause) {
        if (abort.signal.aborted || epoch.current !== generation) return;
        setUnavailable(cause instanceof GatewayApiError && cause.code === "cursor_import_session_unavailable");
        if (++failures >= 5) {
          setPaused(true); setError("状态读取暂不可用；请继续检查同一会话及账号列表，不要重复导入。"); return;
        }
      }
      timer = setTimeout(poll, 1500);
    };
    timer = setTimeout(poll, 500);
    return () => { abort.abort(); clearTimeout(timer); };
  }, [api, operation?.input.attemptId, operation?.session?.status, paused, busy]);
  const start = async (token?: string) => {
    const existing = operations.get(key);
    if (busyRef.current || props.disabled || !props.expectedRevision || !managementToken || existing && active(existing.session) ||
        existing?.session?.status === "save_unconfirmed") return;
    if (!props.secretGrant) { await props.onRequestSecretAccess(); return; }
    // Keep unresolved attempts fenced, but do not let finished records permanently exhaust the bounded map.
    if (operations.size >= 16 && !operations.has(key)) {
      for (const [slot, record] of operations) {
        if (["failed", "cancelled", "expired"].includes(record.session?.status ?? "") ||
          record.session?.status === "succeeded" && record.notified) operations.delete(slot);
      }
    }
    if (operations.size >= 16 && !operations.has(key)) { setError("请先核对现有导入记录。"); return; }
    const input: CursorInput = { attemptId: crypto.randomUUID(), providerId, credentialId,
      expectedRevision: props.expectedRevision };
    // Never retain token material in the operation map, browser storage, URLs or error messages.
    const op: Operation = { input, session: null }, generation = epoch.current;
    operations.set(key, op);
    publish(op); busyRef.current = true; setBusy(true); setError(null); setPaused(false); setUnavailable(false);
    try {
      const session = await api.create({ ...input, ...(token ? { accessToken: token } : {}) }, props.secretGrant);
      if (operations.get(key)?.input.attemptId !== input.attemptId) return;
      operations.set(key, { input, session });
      if (epoch.current === generation) publish({ input, session });
    } catch (cause) {
      const rejected = cause instanceof GatewayApiError && ["cursor_invalid_token", "cursor_token_expired",
        "cursor_import_local_only", "cursor_invalid_expected_email", "cursor_import_target_invalid",
        "console_revision_conflict", "console_secret_access_required", "cursor_import_capacity",
        "cursor_import_stopped", "console_mutation_not_supported"].includes(cause.code ?? "");
      if (operations.get(key)?.input.attemptId !== input.attemptId) return;
      if (rejected) operations.delete(key);
      if (epoch.current !== generation) return;
      if (rejected) setOperation(null);
      setError(rejected ? `未接纳导入：${(cause as GatewayApiError).code}` : "接收结果待确认；仅检查同一会话，不会自动重新授权或提交。");
    } finally {
      if (epoch.current === generation) { busyRef.current = false; setBusy(false); }
    }
  };
  const action = async (name: "cancel" | "resume") => {
    if (!operation || busyRef.current) return;
    if (!props.secretGrant) { await props.onRequestSecretAccess(); return; }
    const generation = epoch.current;
    busyRef.current = true; setBusy(true);
    try {
      const session = await api[name](operation.input, props.secretGrant);
      if (operations.get(key)?.input.attemptId !== operation.input.attemptId) return;
      operations.set(key, { ...operation, session });
      if (epoch.current === generation) publish({ ...operation, session });
    } catch { if (epoch.current === generation) setError("操作未确认，请检查状态；不会自动重新授权或覆盖账号。"); }
    finally { if (epoch.current === generation) { busyRef.current = false; setBusy(false); } }
  };
  const renewPermission = async () => {
    if (renewing.current || busyRef.current) return;
    renewing.current = true;
    const generation = epoch.current;
    setRenewRequested({ before: props.secretGrant, generation });
    try {
      await props.onRequestSecretAccess();
    } catch { if (epoch.current === generation) {
      setRenewRequested(null); setError("Gateway 权限确认未完成；Cursor 已授权事实仍保留，请在会话期限内继续保存。");
    } }
    finally { renewing.current = false; }
  };
  useEffect(() => {
    if (!renewRequested || !props.secretGrant || props.secretGrant === renewRequested.before || busy) return;
    setRenewRequested(null);
    if (epoch.current === renewRequested.generation) void action("resume");
  }, [props.secretGrant, renewRequested, busy]);
  const releaseUnknown = () => {
    if (!(unavailable && paused || operation?.session?.status === "save_unconfirmed") || busyRef.current ||
      operations.get(key)?.input.attemptId !== operation?.input.attemptId) return;
    operations.delete(key); setOperation(null); setUnavailable(false); setPaused(false); setError(null);
  };
  return { session: operation?.session, busy, error, paused, unavailable, releaseUnknown, start,
    cancel: () => action("cancel"), renewPermission,
    pending: Boolean(operation && active(operation.session)),
    resume: () => { setPaused(false); setError(null); } };
}
