import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import type { GrokAuthSession } from "../../api/console/grok-auth";
import { GatewayApiError } from "../../api/errors";
import { useGrokOAuth, type GrokOAuthProps } from "./useGrokOAuth";

const apis = vi.hoisted(() => ({ old: { create: vi.fn(), get: vi.fn(), act: vi.fn() }, next: { create: vi.fn(), get: vi.fn(), act: vi.fn() } }));
vi.mock("../../api/console/grok-auth", () => ({ createGrokAuthApi: (_client: unknown, token: string) => token === "next-owner" ? apis.next : apis.old }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: GrokAuthSession["status"] = "waiting_user"): GrokAuthSession => ({ id: "login", providerId: "build", expectedRevision: "r1", status,
  authorizationUrl: null, identityVerified: false, credentialId: status === "succeeded" ? "account" : null,
  targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const input = { providerId: "build", expectedRevision: "r1", clientId: "permitted", scopes: ["openid"], redirectUri: "http://127.0.0.1:34567/callback" };
const props = (): GrokOAuthProps => ({ client, operations: new Map(), managementToken: "owner", secretGrant: "old-grant", providerId: "build", expectedRevision: "r1",
  disabled: false, onSaved: vi.fn(), onRequestSecretAccess: vi.fn() });
const poll = async (time = 1500) => { await act(async () => { await vi.advanceTimersByTimeAsync(time); }); };
beforeEach(() => {
  vi.useFakeTimers(); vi.resetAllMocks();
  for (const api of Object.values(apis)) { api.create.mockResolvedValue(session()); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); }
});
afterEach(() => vi.useRealTimers());

it("allows explicit reauthorization when the server has reclaimed a pre-save Grok session", async () => {
  apis.old.get.mockRejectedValue(new GatewayApiError("grok_login_unavailable", 400, "grok_login_rejected"));
  const { result } = renderHook(useGrokOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await poll();
  expect(result.current.session).toBeNull();
  expect(result.current.error).toContain("会话已不可用");
  expect(apis.old.get).toHaveBeenCalledTimes(1);
  await act(async () => { await result.current.run({ action: "create", input }); });
  expect(apis.old.create).toHaveBeenCalledTimes(2);
});

it("preserves the duplicate-save fence when a Grok save receipt disappears", async () => {
  const original = props();
  const { result, unmount } = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await poll();
  apis.old.act.mockRejectedValueOnce(new Error("response lost"));
  await act(async () => { await result.current.run({ action: "import" }); });
  apis.old.get.mockRejectedValue(new GatewayApiError("grok_login_unavailable", 400, "grok_login_rejected"));
  await poll();
  expect(result.current.pollStopped).toBe(true);
  expect(result.current.uncertain).toBe(true);
  expect(result.current.error).toContain("保存回执已不可用");
  unmount();
  const reopened = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => {
    await reopened.result.current.run({ action: "create", input });
    await reopened.result.current.run({ action: "import" });
  });
  expect(apis.old.create).toHaveBeenCalledTimes(1);
  expect(apis.old.act).toHaveBeenCalledTimes(1);
  expect(original.operations.size).toBe(1);
});

it("cancels a late create using the original API, grant and revision, never the new owner", async () => {
  let finish!: (value: GrokAuthSession) => void;
  apis.old.create.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const original = props();
  const { result, rerender } = renderHook(useGrokOAuth, { initialProps: original });
  let request!: Promise<void>;
  act(() => { request = result.current.run({ action: "create", input }); });
  rerender({ ...original, managementToken: "next-owner", secretGrant: "new-grant", operations: new Map() });
  await act(async () => { finish(session()); await request; });
  expect(result.current.session).toBeNull();
  expect(apis.old.act).toHaveBeenCalledWith("login", "cancel", input, "old-grant");
  expect(apis.next.act).not.toHaveBeenCalled(); expect(original.onSaved).not.toHaveBeenCalled();
});

it("cleans up an active login with the old owner's latest grant on identity change", async () => {
  const original = props();
  const { result, rerender } = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); });
  rerender({ ...original, secretGrant: "renewed-old-grant" });
  rerender({ ...original, managementToken: "next-owner", secretGrant: "new-grant", operations: new Map() });
  expect(apis.old.act).toHaveBeenCalledWith("login", "cancel", expect.objectContaining({ expectedRevision: "r1" }), "renewed-old-grant");
  expect(apis.next.act).not.toHaveBeenCalled();
});

it("cancels a pending explicit create when its revision changes before permission recovery", async () => {
  const original = { ...props(), secretGrant: null };
  const { result, rerender } = renderHook(useGrokOAuth, { initialProps: original as GrokOAuthProps });
  await act(async () => { await result.current.run({ action: "create", input }); });
  expect(result.current.pending).toBe(true);
  rerender({ ...original, expectedRevision: "r2", secretGrant: "renewed" }); await act(async () => {});
  expect(result.current.pending).toBe(false); expect(apis.old.create).not.toHaveBeenCalled();
});

it("does not let an earlier poll overwrite a newer mutation result", async () => {
  const { result } = renderHook(useGrokOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); }); await poll();
  let finish!: (value: GrokAuthSession) => void;
  apis.old.get.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  await poll();
  apis.old.act.mockResolvedValueOnce(session("importing"));
  await act(async () => { await result.current.run({ action: "import" }); finish(session("ready")); });
  expect(result.current.session?.status).toBe("importing");
});

it("records a save response arriving after unmount and restores it on reopening", async () => {
  const original = props();
  const first = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); }); await poll();
  let finish!: (value: GrokAuthSession) => void;
  apis.old.act.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  let request!: Promise<void>;
  act(() => { request = first.result.current.run({ action: "import" }); }); first.unmount();
  await act(async () => { finish(session("succeeded")); await request; });
  expect(original.onSaved).not.toHaveBeenCalled(); expect(original.operations.size).toBe(1);
  renderHook(useGrokOAuth, { initialProps: original });
  expect(original.onSaved).toHaveBeenCalledOnce(); expect(original.operations.size).toBe(0);
  expect(apis.old.act).toHaveBeenCalledTimes(1);
});

it("stops after five sequential poll failures and resumes only on explicit request", async () => {
  const { result } = renderHook(useGrokOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  apis.old.get.mockRejectedValue(new Error("offline")); await poll(7500); await poll(6000);
  expect(apis.old.get).toHaveBeenCalledTimes(5); expect(result.current.pollStopped).toBe(true);
  act(() => result.current.resumePolling()); await poll(); expect(apis.old.get).toHaveBeenCalledTimes(6);
});

it("does not resurrect a consumed save record when an older import response finally arrives", async () => {
  const original = props();
  const first = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); }); await poll();
  let finish!: (value: GrokAuthSession) => void;
  apis.old.act.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  let request!: Promise<void>;
  act(() => { request = first.result.current.run({ action: "import" }); }); first.unmount();
  renderHook(useGrokOAuth, { initialProps: original });
  apis.old.get.mockResolvedValue(session("succeeded")); await poll();
  expect(original.operations.size).toBe(0); expect(original.onSaved).toHaveBeenCalledOnce();
  await act(async () => { finish(session("importing")); await request; });
  expect(original.operations.size).toBe(0); expect(original.onSaved).toHaveBeenCalledOnce();
});

it("replaces a pending import with cancel instead of replaying import after grant renewal", async () => {
  const original = props();
  const { result, rerender } = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); }); await poll();
  rerender({ ...original, secretGrant: null });
  await act(async () => { await result.current.run({ action: "import" }); await result.current.run({ action: "cancel" }); });
  rerender({ ...original, secretGrant: "renewed" }); await act(async () => {});
  expect(apis.old.act).toHaveBeenCalledExactlyOnceWith("login", "cancel", expect.objectContaining({ expectedRevision: "r1" }), "renewed", undefined);
});

it("restores a late explicit pre-admission rejection to the reopened ready session", async () => {
  const original = props();
  const first = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); }); await poll();
  let reject!: (error: Error) => void;
  apis.old.act.mockReturnValueOnce(new Promise((_resolve, fail) => { reject = fail; }));
  let request!: Promise<void>;
  act(() => { request = first.result.current.run({ action: "import" }); }); first.unmount();
  const second = renderHook(useGrokOAuth, { initialProps: original });
  await act(async () => { reject(new GatewayApiError("Rejected", 400, "grok_login_save_rejected")); await request; });
  await poll();
  expect(second.result.current.uncertain).toBe(false); expect(original.operations.size).toBe(0);
  expect(second.result.current.error).toContain("保存前校验被拒绝"); expect(apis.old.act).toHaveBeenCalledTimes(1);
  apis.old.act.mockResolvedValueOnce(session("succeeded"));
  await act(async () => { await second.result.current.run({ action: "import" }); });
  expect(original.onSaved).toHaveBeenCalledOnce();
});
