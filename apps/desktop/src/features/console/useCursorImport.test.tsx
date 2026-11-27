import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import type { CursorInput, CursorSession } from "../../api/console/cursor-auth";
import { GatewayApiError } from "../../api/errors";
import { useCursorImport, type CursorImportProps } from "./useCursorImport";

const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), cancel: vi.fn(), resume: vi.fn() }));
vi.mock("../../api/console/cursor-auth", () => ({ createCursorAuthApi: () => api }));
const receipt = (input: CursorInput, status: CursorSession["status"] = "waiting_user"): CursorSession => ({
  id: input.attemptId, providerId: input.providerId, expectedRevision: input.expectedRevision,
  targetCredentialId: input.credentialId ?? `cursor-${input.attemptId}`, status, authorizationUrl: null,
  authReceived: status === "succeeded", accountMatched: status === "succeeded", credentialPersisted: status === "succeeded",
  expiresAt: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null, inferenceStatus: "not_checked",
});
const props = (): CursorImportProps => ({ client: {} as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
  providerId: "cursor", expectedRevision: "r1", operations: new Map(), disabled: false, onSaved: vi.fn(), onRequestSecretAccess: vi.fn() });
const deferred = <T,>() => {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
beforeEach(() => {
  vi.useFakeTimers(); vi.resetAllMocks();
  api.create.mockImplementation(async input => receipt(input));
  api.get.mockImplementation(async input => receipt(input, "succeeded"));
});
afterEach(() => vi.useRealTimers());
const poll = async (ms = 1600) => act(async () => { await vi.advanceTimersByTimeAsync(ms); });

it("automatically observes a saved browser session without another authorization or save click", async () => {
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); });
  await poll();
  expect(api.create).toHaveBeenCalledOnce();
  expect(api.create.mock.calls[0][0]).not.toHaveProperty("expectedEmail");
  expect(api.create.mock.calls[0][0]).not.toHaveProperty("accessToken");
  expect(hook.result.current.session?.credentialPersisted).toBe(true);
  expect(hook.result.current.session?.inferenceStatus).toBe("not_checked");
  expect(input.onSaved).toHaveBeenCalledOnce();
  await poll(); expect(input.onSaved).toHaveBeenCalledOnce();
});
it("keeps manual tokens only in the POST, never in the shared operation record", async () => {
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start("synthetic-token"); });
  expect(api.create.mock.calls[0][0].accessToken).toBe("synthetic-token");
  expect(JSON.stringify([...input.operations.values()])).not.toContain("synthetic-token");
  await poll(); expect(api.get.mock.calls[0][0].accessToken).toBeUndefined();
});
it("lost POST response checks only the original attempt and never repeats authorization", async () => {
  api.create.mockRejectedValue(new Error("lost response with private payload"));
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); });
  expect(hook.result.current.error).not.toContain("private payload");
  const attempt = api.create.mock.calls[0][0].attemptId;
  await act(async () => { await hook.result.current.start(); });
  await poll();
  expect(api.create).toHaveBeenCalledOnce();
  expect(api.get.mock.calls[0][0].attemptId).toBe(attempt);
  expect(input.onSaved).toHaveBeenCalledOnce();
});
it("shared synchronous ownership prevents two mounted panels from starting duplicate imports", async () => {
  const pending = deferred<CursorSession>(); api.create.mockReturnValue(pending.promise);
  const input = props(), first = renderHook(useCursorImport, { initialProps: input }), second = renderHook(useCursorImport, { initialProps: input });
  let started!: Promise<void>;
  act(() => { started = first.result.current.start(); });
  await act(async () => { await second.result.current.start(); });
  expect(api.create).toHaveBeenCalledOnce();
  await act(async () => { pending.resolve(receipt(api.create.mock.calls[0][0])); await started; });
});
it("restores background success after unmount and refreshes once across further remounts", async () => {
  const pending = deferred<CursorSession>(); api.create.mockReturnValue(pending.promise);
  const input = props(), first = renderHook(useCursorImport, { initialProps: input });
  let started!: Promise<void>; act(() => { started = first.result.current.start(); });
  first.unmount();
  await act(async () => { pending.resolve(receipt(api.create.mock.calls[0][0], "succeeded")); await started; });
  const second = renderHook(useCursorImport, { initialProps: input });
  expect(input.onSaved).toHaveBeenCalledOnce();
  second.unmount(); renderHook(useCursorImport, { initialProps: input });
  expect(input.onSaved).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce();
});
it("cleans an explicitly rejected create even if its panel has unmounted", async () => {
  const pending = deferred<CursorSession>(); api.create.mockReturnValue(pending.promise);
  const input = props(), first = renderHook(useCursorImport, { initialProps: input });
  let started!: Promise<void>; act(() => { started = first.result.current.start(); });
  first.unmount();
  await act(async () => { pending.reject(new GatewayApiError("bad", 400, "cursor_invalid_token")); await started; });
  expect(input.operations.size).toBe(0);
  const reopened = renderHook(useCursorImport, { initialProps: input });
  expect(reopened.result.current.pending).toBe(false);
});
it("pauses repeated status errors without losing receipt or treating it as logged out", async () => {
  api.get.mockRejectedValue(new Error("network"));
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); });
  await poll(7000);
  expect(hook.result.current.paused).toBe(true); expect(api.get).toHaveBeenCalledTimes(5);
  expect(hook.result.current.pending).toBe(true);
  api.get.mockImplementation(async input => receipt(input, "succeeded"));
  act(() => hook.result.current.resume()); await poll();
  expect(input.onSaved).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce();
});
it("does not rebase an active import when the selected revision or permissions change", async () => {
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); });
  hook.rerender({ ...input, expectedRevision: "r2", disabled: true, secretGrant: null });
  await poll();
  expect(api.get.mock.calls[0][0].expectedRevision).toBe("r1"); expect(input.onSaved).toHaveBeenCalledOnce();
});
it("renews Gateway permission and resumes the same verified attempt without another Cursor login", async () => {
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  api.get.mockImplementation(async input => receipt(input, "awaiting_permission"));
  api.resume.mockImplementation(async input => receipt(input, "succeeded"));
  await act(async () => { await hook.result.current.start(); });
  await poll();
  await act(async () => { await hook.result.current.renewPermission(); });
  expect(input.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(api.resume).not.toHaveBeenCalled();
  hook.rerender({ ...input, secretGrant: "fresh" });
  await act(async () => {});
  expect(api.resume).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ attemptId: api.create.mock.calls[0][0].attemptId }), "fresh");
  expect(api.create).toHaveBeenCalledOnce(); expect(input.onSaved).toHaveBeenCalledOnce();
});
it("requires explicit acknowledgement to release an uncertain save and never auto-reposts", async () => {
  api.get.mockImplementation(async input => receipt(input, "save_unconfirmed"));
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); }); await poll();
  await act(async () => { await hook.result.current.start(); });
  expect(api.create).toHaveBeenCalledOnce();
  act(() => hook.result.current.releaseUnknown());
  expect(input.operations.size).toBe(0); expect(api.create).toHaveBeenCalledOnce();
});
it("successful read clears a stale unavailable marker before later transient network failures", async () => {
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  api.get.mockRejectedValueOnce(new GatewayApiError("unavailable",400,"cursor_import_session_unavailable"))
    .mockImplementationOnce(async input => receipt(input)).mockRejectedValue(new Error("network"));
  await act(async () => { await hook.result.current.start(); }); await poll(2500);
  expect(hook.result.current.unavailable).toBe(false);
  await poll(7500); expect(hook.result.current.paused).toBe(true); expect(hook.result.current.unavailable).toBe(false);
  act(() => hook.result.current.releaseUnknown()); expect(input.operations.size).toBe(1);
});
it("reclaims only settled records at capacity and preserves unknown or unnotified attempts", async () => {
  const input = props();
  for (let i = 0; i < 16; i++) {
    const body = { attemptId: `attempt-${i}`, providerId: `cursor-${i}`, expectedRevision: "r1" };
    input.operations.set(`slot-${i}`, { input: body, session: receipt(body, i === 0 ? "save_unconfirmed" : "succeeded"), notified: i > 1 });
  }
  const hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); });
  expect(api.create).toHaveBeenCalledOnce(); expect(input.operations.size).toBe(3);
  expect(input.operations.has("slot-0")).toBe(true); expect(input.operations.has("slot-1")).toBe(true);
});
it("does not use a stale missing-session error to release a later unknown network result", async () => {
  api.get.mockRejectedValueOnce(new GatewayApiError("unavailable", 400, "cursor_import_session_unavailable"))
    .mockRejectedValue(new Error("network"));
  const input = props(), hook = renderHook(useCursorImport, { initialProps: input });
  await act(async () => { await hook.result.current.start(); }); await poll(7000);
  expect(hook.result.current.paused).toBe(true); expect(hook.result.current.unavailable).toBe(false);
  act(() => hook.result.current.releaseUnknown()); expect(input.operations.size).toBe(1);
});
