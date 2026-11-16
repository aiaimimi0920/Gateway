// Reclaimed pre-save sessions may restart; a missing receipt never proves that a save failed.
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { AuthSession } from "../../api/console/managed-auth";
import { GatewayApiError } from "../../api/errors";
import type { GatewayApiClient } from "../../api/client";
import { useManagedOAuth, type ManagedOAuthProps } from "./useManagedOAuth";

const missing = () => new GatewayApiError("vertex_login_unavailable", 400, "vertex_login_rejected");
const policy = { polling: (status: string) => ["waiting_user", "ready", "importing"].includes(status), rejected: [],
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "vertex_login_rejected" && error.message === "vertex_login_unavailable" };
const session = (status = "waiting_user"): AuthSession => ({ id: "login", providerId: "vertex", expectedRevision: "r1", status,
  credentialId: null, targetCredentialId: null, savedRevision: null });
const api = { create: vi.fn(), get: vi.fn(), act: vi.fn() }; const createApi = () => api;
const input = { providerId: "vertex", expectedRevision: "r1" };
const props = (): ManagedOAuthProps<AuthSession> => ({ client: {} as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
  ...input, disabled: false, operations: new Map(), onRequestSecretAccess: vi.fn(), onSaved: vi.fn() });
const useFlow = (p: ManagedOAuthProps<AuthSession>) => useManagedOAuth(p, createApi, policy);
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session()); api.get.mockRejectedValue(missing()); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => vi.useRealTimers());
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
it("stops on a definitive missing pre-save session and permits only explicit restart", async () => {
  const original = props(); const { result } = renderHook(useFlow, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); }); await poll();
  expect(result.current.session).toBeNull(); expect(result.current.active).toBe(false);
  expect(result.current.error).toContain("会话已不可用"); expect(api.get).toHaveBeenCalledTimes(1);
  await act(async () => { await result.current.run({ action: "create", input }); }); expect(api.create).toHaveBeenCalledTimes(2);
});
it("preserves an uncertain save across a missing receipt and reopening without submitting twice", async () => {
  api.get.mockResolvedValueOnce(session("ready")); api.act.mockRejectedValueOnce(new Error("lost"));
  const original = props(); const first = renderHook(useFlow, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); }); await poll();
  await act(async () => { await first.result.current.run({ action: "import" }); }); await poll();
  expect(first.result.current.uncertain).toBe(true); expect(first.result.current.pollStopped).toBe(true);
  expect(original.operations.size).toBe(1); first.unmount();
  const next = renderHook(useFlow, { initialProps: original });
  await act(async () => { await next.result.current.run({ action: "import" }); await next.result.current.run({ action: "create", input }); });
  await poll(); expect(next.result.current.uncertain).toBe(true); expect(api.create).toHaveBeenCalledTimes(1); expect(api.act).toHaveBeenCalledTimes(1);
  act(() => next.result.current.resumePolling()); await poll(); expect(api.act).toHaveBeenCalledTimes(1);
});
