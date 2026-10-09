import { useEffect, useRef, useState } from "react";
import type { ConsoleApi } from "../api/console";
import type { SecretGrant } from "../api/contracts";
import { isAuthenticationError } from "../api/errors";

type Options = {
  api: ConsoleApi;
  token: string | null;
  enabled: boolean;
  grant: SecretGrant | null;
  onGrant(grant: SecretGrant | null): void;
  onError(message: string | null): void;
  onAuthenticationFailure(): void;
};
const MAX_TIMER_MS = 2_147_483_647;
const RETRY_MS = 30_000;
const MIN_RENEW_MS = 1_000;

/** Reuse the initial login credential; grants stay short-lived and memory-only. */
export function useAutomaticSecretAccess({ api, token, enabled, grant, onGrant, onError, onAuthenticationFailure }: Options) {
  const [busy, setBusy] = useState(false);
  const latestGrant = useRef(grant);
  const controller = useRef<{ update(value: SecretGrant | null): void } | null>(null);
  useEffect(() => { latestGrant.current = grant; controller.current?.update(grant); }, [grant]);
  useEffect(() => {
    let active = true;
    let inFlight = false;
    let nextAttemptAt = 0;
    let current = latestGrant.current;
    let renewalTimer: ReturnType<typeof setTimeout> | undefined;
    let expiryTimer: ReturnType<typeof setTimeout> | undefined;
    setBusy(false);
    if (!enabled || !token) return;
    const scheduleRenewal = () => {
      if (renewalTimer !== undefined) clearTimeout(renewalTimer);
      if (!active || inFlight) return;
      const remaining = current ? Date.parse(current.expiresAt) - Date.now() : 0;
      const renewIn = Number.isFinite(remaining) && remaining > 0 ? remaining - Math.min(30_000, remaining / 10) : 0;
      const wait = Math.max(renewIn, nextAttemptAt - Date.now(), 0);
      renewalTimer = setTimeout(() => {
        if (wait > MAX_TIMER_MS) scheduleRenewal(); else void renew();
      }, Math.min(wait, MAX_TIMER_MS));
    };
    const scheduleExpiry = () => {
      if (expiryTimer !== undefined) clearTimeout(expiryTimer);
      if (!active || !current) return;
      const remaining = Date.parse(current.expiresAt) - Date.now();
      if (!Number.isFinite(remaining) || remaining <= 0) {
        current = null; onGrant(null); scheduleRenewal(); return;
      }
      // Expiry is independent of renewal: a stalled/failed request cannot keep access true.
      expiryTimer = setTimeout(scheduleExpiry, Math.min(remaining, MAX_TIMER_MS));
    };
    const renew = async () => {
      if (!active || inFlight) return;
      inFlight = true;
      nextAttemptAt = Date.now() + MIN_RENEW_MS;
      setBusy(true);
      try {
        const next = await api.confirmSecretAccess(token, token);
        if (!active) return;
        const expiry = Date.parse(next.expiresAt);
        if (!next.grant || !Number.isFinite(expiry) || expiry <= Date.now()) throw new Error("管理会话授权暂不可用，将自动重试。");
        current = next; onGrant(next); onError(null); scheduleExpiry();
      } catch (cause) {
        if (!active) return;
        if (isAuthenticationError(cause)) { active = false; onAuthenticationFailure(); return; }
        nextAttemptAt = Date.now() + RETRY_MS;
        onError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        inFlight = false;
        if (active) { setBusy(false); scheduleRenewal(); }
      }
    };
    const owner = { update(value: SecretGrant | null) { current = value; scheduleExpiry(); scheduleRenewal(); } };
    controller.current = owner;
    scheduleExpiry();
    if (!current) void renew(); else scheduleRenewal();
    return () => {
      active = false;
      if (renewalTimer !== undefined) clearTimeout(renewalTimer);
      if (expiryTimer !== undefined) clearTimeout(expiryTimer);
      if (controller.current === owner) controller.current = null;
    };
  }, [api, token, enabled, onGrant, onError, onAuthenticationFailure]);
  return busy;
}
