import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useAutomaticSecretAccess } from "./useAutomaticSecretAccess";
import { createApi, deferred } from "./managementSessionTestFixtures";
import type { SecretGrant } from "../api/contracts";

const grant = (): SecretGrant => ({ grant: "memory-only", expiresAt: new Date(Date.now() + 600_000).toISOString() });
afterEach(() => vi.useRealTimers());
function options() {
  return { api: createApi(), token: "login-token", enabled: true, grant: null as SecretGrant | null,
    onGrant: vi.fn(), onError: vi.fn(), onAuthenticationFailure: vi.fn() };
}

it("reuses the first login credential without asking for another password", async () => {
  const p = options(); vi.mocked(p.api.confirmSecretAccess).mockResolvedValue(grant());
  const hook = renderHook(() => useAutomaticSecretAccess(p));
  await act(async () => {});
  expect(p.api.confirmSecretAccess).toHaveBeenCalledExactlyOnceWith("login-token", "login-token");
  expect(p.onGrant).toHaveBeenCalledWith(expect.objectContaining({ grant: "memory-only" }));
  expect(hook.result.current).toBe(false);
});

it("renews before expiry and removes its timer on unmount", async () => {
  vi.useFakeTimers(); const p = { ...options(), grant: grant() as SecretGrant | null };
  const hook = renderHook(() => useAutomaticSecretAccess(p));
  expect(p.api.confirmSecretAccess).not.toHaveBeenCalled();
  await act(async () => { await vi.advanceTimersByTimeAsync(570_000); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledOnce();
  hook.unmount(); await vi.advanceTimersByTimeAsync(600_000);
  expect(p.api.confirmSecretAccess).toHaveBeenCalledOnce();
});

it("bounds transport retries and ignores grants from a prior login identity", async () => {
  vi.useFakeTimers(); const p = options();
  const old = deferred<SecretGrant>();
  vi.mocked(p.api.confirmSecretAccess).mockReturnValueOnce(old.promise).mockRejectedValue(new Error("offline"));
  const hook = renderHook((props) => useAutomaticSecretAccess(props), { initialProps: p });
  hook.rerender({ ...p, token: "new-token" });
  await act(async () => { old.resolve(grant()); });
  expect(p.onGrant).not.toHaveBeenCalled();
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(2);
  await act(async () => { await vi.advanceTimersByTimeAsync(29_999); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(2);
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(3);
  hook.unmount(); await vi.advanceTimersByTimeAsync(60_000);
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(3);
});

it("does not overlap a stalled renewal with expiry and accepts its current-identity reply", async () => {
  vi.useFakeTimers(); const p = { ...options(), grant: grant() as SecretGrant | null };
  const pending = deferred<SecretGrant>();
  vi.mocked(p.api.confirmSecretAccess).mockReturnValue(pending.promise);
  const hook = renderHook((props) => useAutomaticSecretAccess(props), { initialProps: p });
  await act(async () => { await vi.advanceTimersByTimeAsync(600_001); });
  expect(p.onGrant).toHaveBeenCalledWith(null);
  expect(p.api.confirmSecretAccess).toHaveBeenCalledOnce();
  hook.rerender({ ...p, grant: null });
  await act(async () => { await vi.advanceTimersByTimeAsync(60_000); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledOnce();
  await act(async () => { pending.resolve(grant()); });
  expect(p.onGrant).toHaveBeenLastCalledWith(expect.objectContaining({ grant: "memory-only" }));
});

it("bounds tiny TTL renewal and invalid-grant retries instead of spinning", async () => {
  vi.useFakeTimers(); const p = options();
  vi.mocked(p.api.confirmSecretAccess).mockImplementation(async () => ({ grant: "tiny", expiresAt: new Date(Date.now() + 10).toISOString() }));
  const hook = renderHook(() => useAutomaticSecretAccess(p));
  await act(async () => { await vi.advanceTimersByTimeAsync(999); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledOnce();
  expect(p.onGrant).toHaveBeenLastCalledWith(null);
  vi.mocked(p.api.confirmSecretAccess).mockResolvedValue({ grant: "expired", expiresAt: new Date(0).toISOString() });
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(2);
  await act(async () => { await vi.advanceTimersByTimeAsync(29_999); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(2);
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(p.api.confirmSecretAccess).toHaveBeenCalledTimes(3);
  hook.unmount();
});
