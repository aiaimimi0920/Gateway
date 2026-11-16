import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { FreebuffAuthSession } from "../../api/console/freebuff-auth";
import { useFreebuffLogin, type FreebuffLoginProps } from "./useFreebuffLogin";

const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/freebuff-auth", () => ({ createFreebuffAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: FreebuffAuthSession["status"] = "waiting_user"): FreebuffAuthSession => ({ id: "login", providerId: "freebuff", expectedRevision: "r1", status,
  loginUrl: null, pollIntervalSecs: null, expiresInSecs: null, credentialId: status === "succeeded" ? "account" : null,
  targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const input = { providerId: "freebuff", expectedRevision: "r1" };
const props = (): FreebuffLoginProps => ({ client, operations: new Map(), managementToken: "owner", secretGrant: "grant", ...input,
  disabled: false, onSaved: vi.fn(), onRequestSecretAccess: vi.fn() });
const check = async (time = 1500) => { await act(async () => { await vi.advanceTimersByTimeAsync(time); }); };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session()); api.get.mockResolvedValue(session()); api.act.mockResolvedValue(session("polling")); });
afterEach(() => vi.useRealTimers());

it("rejects callback completion and serializes explicit approval polls", async () => {
  const { result } = renderHook(useFreebuffLogin, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); await result.current.run({ action: "complete", callbackUrl: "synthetic-callback" }); });
  expect(api.act).not.toHaveBeenCalled();
  let finish!: (value: FreebuffAuthSession) => void; api.act.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  let request!: Promise<void>; act(() => { request = result.current.run({ action: "poll" }); });
  await act(async () => { await result.current.run({ action: "poll" }); }); expect(api.act).toHaveBeenCalledOnce();
  await act(async () => { finish(session("polling")); await request; await result.current.run({ action: "poll" }); }); expect(api.act).toHaveBeenCalledOnce();
});

it.each(["starting", "polling", "ready", "importing", "failed", "cancelled", "expired", "save_unconfirmed"] as const)("refuses approval polling outside waiting_user: %s", async (status) => {
  api.create.mockResolvedValueOnce(session(status)); const { result } = renderHook(useFreebuffLogin, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); await result.current.run({ action: "poll" }); }); expect(api.act).not.toHaveBeenCalled();
});

it("restores only the captured explicit action after a new grant", async () => {
  const owner = props(); const { result, rerender } = renderHook(useFreebuffLogin, { initialProps: owner });
  await act(async () => { await result.current.run({ action: "create", input }); }); rerender({ ...owner, secretGrant: null });
  await act(async () => { await result.current.run({ action: "poll" }); }); expect(api.act).not.toHaveBeenCalled(); expect(owner.onRequestSecretAccess).toHaveBeenCalledOnce();
  rerender({ ...owner, secretGrant: "renewed" }); await act(async () => {});
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "poll", expect.objectContaining({ expectedRevision: "r1" }), "renewed", undefined);
  await check(); expect(api.act).toHaveBeenCalledOnce();
});

it("revokes a captured action on revision change instead of rebasing it", async () => {
  const owner = props(); const { result, rerender } = renderHook(useFreebuffLogin, { initialProps: owner });
  await act(async () => { await result.current.run({ action: "create", input }); }); rerender({ ...owner, secretGrant: null });
  await act(async () => { await result.current.run({ action: "poll" }); }); rerender({ ...owner, secretGrant: "renewed", expectedRevision: "r2" });
  await act(async () => {}); expect(api.act).not.toHaveBeenCalled(); expect(result.current.stale).toBe(true); expect(result.current.pending).toBe(false);
});

it("does not let a stale GET overwrite an explicit poll result", async () => {
  const { result } = renderHook(useFreebuffLogin, { initialProps: props() }); await act(async () => { await result.current.run({ action: "create", input }); });
  let finish!: (value: FreebuffAuthSession) => void; api.get.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; })); await check();
  api.act.mockResolvedValueOnce(session("ready")); await act(async () => { await result.current.run({ action: "poll" }); finish(session()); });
  expect(result.current.session?.status).toBe("ready");
});

it("continues reading an accepted save after revision changes without importing again", async () => {
  const owner = props(); api.create.mockResolvedValueOnce(session("ready")); const { result, rerender, unmount } = renderHook(useFreebuffLogin, { initialProps: owner });
  await act(async () => { await result.current.run({ action: "create", input }); }); api.act.mockResolvedValueOnce(session("importing"));
  await act(async () => { await result.current.run({ action: "import" }); }); rerender({ ...owner, expectedRevision: "r2", disabled: true });
  api.get.mockResolvedValue(session("succeeded")); await check(); expect(owner.onSaved).toHaveBeenCalledOnce(); unmount(); expect(api.act).toHaveBeenCalledOnce();
});

it("bounds failed reads and resumes GET only, without repeating approval or saving", async () => {
  const { result } = renderHook(useFreebuffLogin, { initialProps: props() }); await act(async () => { await result.current.run({ action: "create", input }); });
  api.get.mockRejectedValue(new Error("offline")); await check(13500); expect(result.current.pollStopped).toBe(true); expect(api.get).toHaveBeenCalledTimes(5);
  act(() => result.current.resumePolling()); await check(); expect(api.get).toHaveBeenCalledTimes(6); expect(api.act).not.toHaveBeenCalled();
});

it.each([false, true])("treats a vanished receipt differently before/after save admission: saved=%s", async (accepted) => {
  const owner = props(); const { result } = renderHook(useFreebuffLogin, { initialProps: owner }); await act(async () => { await result.current.run({ action: "create", input }); });
  if (accepted) { api.get.mockResolvedValue(session("ready")); await check(); api.act.mockResolvedValueOnce(session("importing")); await act(async () => { await result.current.run({ action: "import" }); }); }
  api.get.mockRejectedValue(new GatewayApiError("freebuff_login_unavailable", 400, "freebuff_login_rejected")); await check();
  expect(result.current.uncertain).toBe(accepted); expect(result.current.pollStopped).toBe(accepted); expect(Boolean(result.current.session)).toBe(accepted);
  expect(owner.operations.size).toBe(accepted ? 1 : 0); expect(owner.onSaved).not.toHaveBeenCalled();
});
