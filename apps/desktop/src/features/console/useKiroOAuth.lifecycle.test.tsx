import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { KiroAuthSession } from "../../api/console/kiro-auth";
import { useKiroOAuth, type KiroOAuthProps } from "./useKiroOAuth";

const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/kiro-auth", () => ({ createKiroAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: KiroAuthSession["status"] = "waiting_user"): KiroAuthSession => ({ id: "login", providerId: "kiro", expectedRevision: "r1", status,
  verificationUri: null, verificationUriComplete: null, userCode: null, pollIntervalSecs: null, expiresInSecs: null,
  credentialId: status === "succeeded" ? "account" : null, targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const input = { providerId: "kiro", expectedRevision: "r1", directory: { authMethod: "builder_id" as const }, scopes: ["codewhisperer:conversations" as const], machineId: "a".repeat(64) };
const props = (): KiroOAuthProps => ({ client, operations: new Map(), managementToken: "owner", secretGrant: "grant", providerId: "kiro", expectedRevision: "r1",
  disabled: false, onSaved: vi.fn(), onRequestSecretAccess: vi.fn() });
const check = async (time = 1500) => { await act(async () => { await vi.advanceTimersByTimeAsync(time); }); };
beforeEach(() => {
  vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session()); api.get.mockResolvedValue(session()); api.act.mockResolvedValue(session("polling"));
});
afterEach(() => vi.useRealTimers());

it("recovers a missing pre-save session without automatically creating another login", async () => {
  api.get.mockRejectedValue(new GatewayApiError("kiro_login_unavailable", 400, "kiro_login_rejected"));
  const { result } = renderHook(useKiroOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await check();
  expect(result.current.session).toBeNull(); expect(result.current.active).toBe(false);
  expect(result.current.error).toContain("会话已不可用");
  expect(api.get).toHaveBeenCalledTimes(1); expect(api.create).toHaveBeenCalledTimes(1);
  await act(async () => { await result.current.run({ action: "create", input }); });
  expect(api.create).toHaveBeenCalledTimes(2);
});

it("keeps an accepted save fenced when its receipt is unavailable after reopening", async () => {
  api.create.mockResolvedValueOnce(session("ready"));
  api.act.mockRejectedValueOnce(new Error("response lost"));
  const original = props(); const first = renderHook(useKiroOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); });
  await act(async () => { await first.result.current.run({ action: "import" }); });
  api.get.mockRejectedValue(new GatewayApiError("kiro_login_unavailable", 400, "kiro_login_rejected"));
  await check();
  expect(first.result.current.pollStopped).toBe(true); expect(first.result.current.uncertain).toBe(true);
  expect(first.result.current.error).toContain("保存回执已不可用");
  first.unmount();
  const second = renderHook(useKiroOAuth, { initialProps: original });
  await act(async () => {
    await second.result.current.run({ action: "create", input });
    await second.result.current.run({ action: "import" });
  });
  expect(original.operations.size).toBe(1); expect(original.onSaved).not.toHaveBeenCalled();
  expect(api.create).toHaveBeenCalledTimes(1); expect(api.act).toHaveBeenCalledTimes(1);
});

it.each([
  ["kiro_login_capacity", "kiro_login_rejected"],
  ["kiro_login_unavailable", "console_revision_conflict"],
])("does not mistake another backend rejection for a missing session: %s / %s", async (message, code) => {
  api.get.mockRejectedValue(new GatewayApiError(message, 400, code));
  const { result } = renderHook(useKiroOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await check();
  expect(result.current.session?.id).toBe("login"); expect(result.current.active).toBe(true);
  expect(result.current.pollStopped).toBe(false); expect(api.create).toHaveBeenCalledTimes(1);
});


it("does not accept callback completion and serializes explicit device polls", async () => {
  const { result } = renderHook(useKiroOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await act(async () => { await result.current.run({ action: "complete", callbackUrl: "not-a-device-flow" }); });
  expect(api.act).not.toHaveBeenCalled();
  let finish!: (value: KiroAuthSession) => void; api.act.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  let request!: Promise<void>; act(() => { request = result.current.run({ action: "poll" }); });
  await act(async () => { await result.current.run({ action: "poll" }); });
  expect(api.act).toHaveBeenCalledOnce(); await act(async () => { finish(session("polling")); await request; });
  await act(async () => { await result.current.run({ action: "poll" }); }); expect(api.act).toHaveBeenCalledOnce();
});

it.each(["registering", "ready", "failed", "cancelled", "expired"] as const)("refuses device polling outside waiting_user: %s", async (status) => {
  api.create.mockResolvedValueOnce(session(status)); const { result } = renderHook(useKiroOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); await result.current.run({ action: "poll" }); });
  expect(api.act).not.toHaveBeenCalled();
});

it("resumes only the captured explicit poll with a renewed grant", async () => {
  const original = props(); const { result, rerender } = renderHook(useKiroOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); });
  rerender({ ...original, secretGrant: null }); await act(async () => { await result.current.run({ action: "poll" }); });
  expect(api.act).not.toHaveBeenCalled(); expect(original.onRequestSecretAccess).toHaveBeenCalledOnce();
  rerender({ ...original, secretGrant: "renewed" }); await act(async () => {});
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "poll", expect.objectContaining({ expectedRevision: "r1" }), "renewed", undefined);
  await check(); expect(api.act).toHaveBeenCalledOnce();
});

it("does not let a stale GET overwrite an explicit device-poll result", async () => {
  const { result } = renderHook(useKiroOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  let finish!: (value: KiroAuthSession) => void; api.get.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  await check(); api.act.mockResolvedValueOnce(session("ready"));
  await act(async () => { await result.current.run({ action: "poll" }); finish(session()); });
  expect(result.current.session?.status).toBe("ready"); expect(api.act).toHaveBeenCalledOnce();
});

it("keeps reading an accepted save across revision changes but never reimports it", async () => {
  const original = props(); api.create.mockResolvedValueOnce(session("ready"));
  const { result, rerender, unmount } = renderHook(useKiroOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); });
  api.act.mockResolvedValueOnce(session("importing")); await act(async () => { await result.current.run({ action: "import" }); });
  rerender({ ...original, expectedRevision: "r2", disabled: true }); api.get.mockResolvedValue(session("succeeded")); await check();
  expect(original.onSaved).toHaveBeenCalledOnce(); unmount(); expect(api.act).toHaveBeenCalledOnce();
});

it("bounds failed GET reads and resumes GET without repeating a device poll", async () => {
  const { result } = renderHook(useKiroOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  api.get.mockRejectedValue(new Error("offline")); await check(13500);
  expect(result.current.pollStopped).toBe(true); expect(api.get).toHaveBeenCalledTimes(5); expect(api.act).not.toHaveBeenCalled();
  act(() => result.current.resumePolling()); await check(); expect(api.get).toHaveBeenCalledTimes(6); expect(api.act).not.toHaveBeenCalled();
});
