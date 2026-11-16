// A confirmed refresh outlives its HTTP request; resume only saves already received evidence.
import { useEffect, useMemo, useRef, useState } from "react";
import { createManagedAuthApi, type AuthBinding } from "../../api/console/managed-auth";
import { lumaAuthSchema, type LumaAuthSession } from "../../api/console/luma-auth";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { GatewayApiError } from "../../api/errors";
import { z } from "zod";

const root = "/v1/internal/gateway/console/luma-refresh-sessions";
const refreshSchema = lumaAuthSchema.extend({ session: lumaAuthSchema.shape.session.extend({
  status: z.enum(["ready", "importing", "succeeded", "save_unconfirmed", "cancelled", "expired"]),
}) });
export function useLumaRefresh(props: LumaOAuthProps) {
  const { client, managementToken, providerId, credentialId, operations } = props;
  const key = JSON.stringify(["luma-refresh", providerId, credentialId]);
  const api = useMemo(() => createManagedAuthApi<AuthBinding, LumaAuthSession>(client, managementToken ?? "", root, refreshSchema), [client, managementToken]);
  const [session, setSession] = useState<LumaAuthSession | null>(null);
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState(false);
  const [error, setError] = useState(false);
  const [pollVersion, setPollVersion] = useState(0);
  const [pollStopped, setPollStopped] = useState(false);
  const current = useRef(props); current.current = props;
  const generation = useRef(0), running = useRef(false), delivered = useRef<string | null>(null);
  const accept = (value: LumaAuthSession, epoch: number, ambiguous = false) => {
    operations.set(key, { session: value, uncertain: ambiguous });
    if (generation.current !== epoch) return;
    setSession(value); setUncertain(ambiguous);
    if (value.status === "cancelled" || value.status === "expired") { operations.delete(key); setSession(null); setUncertain(false); }
    if (value.status === "succeeded" && delivered.current !== value.id) {
      delivered.current = value.id; operations.delete(key); setError(false); current.current.onSaved();
    }
  };
  useEffect(() => {
    generation.current += 1; running.current = false; delivered.current = null;
    setBusy(false); setError(false); setPollStopped(false);
    const retained = operations.get(key);
    setSession(retained?.session ?? null); setUncertain(retained?.uncertain ?? false);
    if (retained) accept(retained.session, generation.current, retained.uncertain);
    return () => { generation.current += 1; };
  }, [api, key, operations]);
  const shouldPoll = session?.status === "importing" || uncertain;
  useEffect(() => {
    if (!session || !shouldPoll || busy || pollStopped) return;
    const controller = new AbortController(), epoch = generation.current;
    let timer: ReturnType<typeof setTimeout>, failures = 0;
    const poll = async () => {
      try {
        const value = await api.get(session.id, { providerId, credentialId, expectedRevision: session.expectedRevision }, controller.signal);
        if (controller.signal.aborted || generation.current !== epoch) return;
        // Ready after a lost import response does not prove that no request was accepted.
        const ambiguous = uncertain && value.status === "ready";
        accept(value, epoch, ambiguous); failures = 0;
        if (value.status !== "importing" && !ambiguous) return;
      } catch {
        if (controller.signal.aborted || generation.current !== epoch) return;
        if (++failures >= 5) { setPollStopped(true); setError(true); return; }
      }
      timer = setTimeout(poll, 1500);
    };
    timer = setTimeout(poll, 1500);
    return () => { controller.abort(); clearTimeout(timer); };
  }, [api, session?.id, session?.status, shouldPoll, busy, pollVersion, pollStopped]);

  const run = async (action: "prepare" | "import" | "resume" | "cancel") => {
    if (running.current || !managementToken) return;
    if (action === "prepare" || action === "import") {
      if (props.disabled || !props.expectedRevision || uncertain) return;
    }
    if (action === "prepare" && session && session.status !== "succeeded") return;
    if (action === "import" && session?.expectedRevision !== props.expectedRevision) return;
    if (action !== "prepare" && !session) return;
    if (action === "import" && session?.status !== "ready") return;
    if (action === "resume" && session?.status !== "save_unconfirmed" && !uncertain) return;
    if (!props.secretGrant) {
      const epoch = generation.current;
      try { await props.onRequestSecretAccess(); }
      catch { if (generation.current === epoch) setError(true); }
      return;
    }
    if (!operations.has(key) && operations.size >= 16) { setError(true); return; }
    const epoch = generation.current;
    running.current = true; setBusy(true); setError(false); setPollStopped(false);
    const binding = { providerId, credentialId, expectedRevision: action === "prepare" ? props.expectedRevision : session?.expectedRevision ?? props.expectedRevision };
    if (session && (action === "import" || action === "resume")) {
      operations.set(key, { session, uncertain: true }); setUncertain(true);
    }
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30_000);
    try {
      const value = action === "prepare" ? await api.create(binding, props.secretGrant)
        : action === "resume" ? (await client.request(`${root}/${encodeURIComponent(session!.id)}/resume`, refreshSchema, {
          method: "POST", cache: "no-store", managementToken, secretGrant: props.secretGrant, signal: controller.signal,
          body: { expectedRevision: binding.expectedRevision },
        })).session : await api.act(session!.id, action, binding, props.secretGrant);
      if (value.providerId !== providerId || value.expectedRevision !== binding.expectedRevision
        || action !== "prepare" && value.id !== session!.id
        || value.targetCredentialId !== credentialId
        || value.status === "succeeded" && (value.credentialId !== credentialId || !value.savedRevision)) throw new Error("Refresh binding mismatch");
      accept(value, epoch);
    } catch (cause) {
      const rejected = cause instanceof GatewayApiError && ["console_secret_access_required", "luma_login_rejected", "console_revision_conflict", "console_mutation_not_supported"].includes(cause.code ?? "");
      if (action === "import" && rejected && session) operations.set(key, { session, uncertain: false });
      if (generation.current === epoch) {
        setError(true);
        if (action === "import" && rejected) setUncertain(false);
        if (cause instanceof GatewayApiError && cause.code === "console_secret_access_required") {
          try { await current.current.onRequestSecretAccess(); } catch { setError(true); }
        }
      }
    } finally {
      clearTimeout(timeout);
      if (generation.current === epoch) { running.current = false; setBusy(false); }
    }
  };
  return { session, busy, uncertain, error, pollStopped, run,
    resumePolling: () => { setError(false); setPollStopped(false); setPollVersion((v) => v + 1); } };
}
