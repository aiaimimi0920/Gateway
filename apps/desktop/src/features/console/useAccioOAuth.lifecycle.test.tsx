import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { AccioAuthSession } from "../../api/console/accio-auth";
import { useAccioOAuth, type AccioOAuthProps } from "./useAccioOAuth";

const apis = vi.hoisted(() => ({ old: { create: vi.fn(), get: vi.fn(), act: vi.fn() }, next: { create: vi.fn(), get: vi.fn(), act: vi.fn() } }));
vi.mock("../../api/console/accio-auth", () => ({ createAccioAuthApi: (_client: unknown, token: string) => token === "next-owner" ? apis.next : apis.old }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: AccioAuthSession["status"] = "waiting_user"): AccioAuthSession => ({ id: "login", providerId: "accio", expectedRevision: "r1", status,
  authorizationUrl: null, credentialId: status === "succeeded" ? "account" : null, targetCredentialId: null,
  savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const input = { providerId: "accio", expectedRevision: "r1", clientId: "permitted", loginSite: "china" as const, callbackPort: 34567, utdid: "device" };
const props = (): AccioOAuthProps => ({ client, operations: new Map(), managementToken: "owner", secretGrant: "old-grant", providerId: "accio", expectedRevision: "r1",
  disabled: false, onSaved: vi.fn(), onRequestSecretAccess: vi.fn() });
const poll = async (time = 1500) => { await act(async () => { await vi.advanceTimersByTimeAsync(time); }); };
beforeEach(() => {
  vi.useFakeTimers(); vi.resetAllMocks();
  for (const api of Object.values(apis)) { api.create.mockResolvedValue(session()); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); }
});
afterEach(() => vi.useRealTimers());

it("recovers a missing pre-save session without automatically creating another login", async () => {
  apis.old.get.mockRejectedValue(new GatewayApiError("accio_login_unavailable", 400, "accio_login_rejected"));
  const { result } = renderHook(useAccioOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await poll();
  expect(result.current.session).toBeNull(); expect(result.current.active).toBe(false);
  expect(result.current.error).toContain("会话已不可用");
  expect(apis.old.get).toHaveBeenCalledTimes(1); expect(apis.old.create).toHaveBeenCalledTimes(1);
  await act(async () => { await result.current.run({ action: "create", input }); });
  expect(apis.old.create).toHaveBeenCalledTimes(2);
});

it("keeps an accepted save fenced when its receipt is unavailable after reopening", async () => {
  apis.old.create.mockResolvedValueOnce(session("ready"));
  apis.old.act.mockRejectedValueOnce(new Error("response lost"));
  const original = props(); const first = renderHook(useAccioOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); });
  await act(async () => { await first.result.current.run({ action: "import" }); });
  apis.old.get.mockRejectedValue(new GatewayApiError("accio_login_unavailable", 400, "accio_login_rejected"));
  await poll();
  expect(first.result.current.pollStopped).toBe(true); expect(first.result.current.uncertain).toBe(true);
  expect(first.result.current.error).toContain("保存回执已不可用");
  first.unmount();
  const second = renderHook(useAccioOAuth, { initialProps: original });
  await act(async () => {
    await second.result.current.run({ action: "create", input });
    await second.result.current.run({ action: "import" });
  });
  expect(original.operations.size).toBe(1); expect(original.onSaved).not.toHaveBeenCalled();
  expect(apis.old.create).toHaveBeenCalledTimes(1); expect(apis.old.act).toHaveBeenCalledTimes(1);
});

it.each([
  ["accio_login_capacity", "accio_login_rejected"],
  ["accio_login_unavailable", "console_revision_conflict"],
])("does not mistake another backend rejection for a missing session: %s / %s", async (message, code) => {
  apis.old.get.mockRejectedValue(new GatewayApiError(message, 400, code));
  const { result } = renderHook(useAccioOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  await poll();
  expect(result.current.session?.id).toBe("login"); expect(result.current.active).toBe(true);
  expect(result.current.pollStopped).toBe(false); expect(apis.old.create).toHaveBeenCalledTimes(1);
});


it("cancels a late create with its original identity and grant", async () => {
  let finish!: (value: AccioAuthSession) => void;
  apis.old.create.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const original = props(); const { result, rerender } = renderHook(useAccioOAuth, { initialProps: original });
  let request!: Promise<void>; act(() => { request = result.current.run({ action: "create", input }); });
  rerender({ ...original, managementToken: "next-owner", secretGrant: "new-grant", operations: new Map() });
  await act(async () => { finish(session()); await request; });
  expect(result.current.session).toBeNull(); expect(apis.old.act).toHaveBeenCalledWith("login", "cancel", input, "old-grant");
  expect(apis.next.act).not.toHaveBeenCalled(); expect(original.onSaved).not.toHaveBeenCalled();
});

it("cancels an unsaved login with the previous owner's latest grant", async () => {
  const original = props(); const { result, rerender } = renderHook(useAccioOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); });
  rerender({ ...original, secretGrant: "renewed-old-grant" });
  rerender({ ...original, managementToken: "next-owner", secretGrant: "new-grant", operations: new Map() });
  expect(apis.old.act).toHaveBeenCalledWith("login", "cancel", expect.objectContaining({ expectedRevision: "r1" }), "renewed-old-grant");
  expect(apis.next.act).not.toHaveBeenCalled();
});

it("does not rebase a pending create after grant recovery", async () => {
  const original = { ...props(), secretGrant: null }; const { result, rerender } = renderHook(useAccioOAuth, { initialProps: original as AccioOAuthProps });
  await act(async () => { await result.current.run({ action: "create", input }); });
  rerender({ ...original, expectedRevision: "r2", secretGrant: "renewed" }); await act(async () => {});
  expect(result.current.pending).toBe(false); expect(apis.old.create).not.toHaveBeenCalled();
});

it("discards a poll overtaken by a newer save mutation", async () => {
  const { result } = renderHook(useAccioOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); }); await poll();
  let finish!: (value: AccioAuthSession) => void;
  apis.old.get.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; })); await poll();
  apis.old.act.mockResolvedValueOnce(session("importing"));
  await act(async () => { await result.current.run({ action: "import" }); finish(session("ready")); });
  expect(result.current.session?.status).toBe("importing");
});

it("restores late save success after unmount without creating or submitting twice", async () => {
  const original = props(); const first = renderHook(useAccioOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); }); await poll();
  let finish!: (value: AccioAuthSession) => void;
  apis.old.act.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  let request!: Promise<void>; act(() => { request = first.result.current.run({ action: "import" }); }); first.unmount();
  await act(async () => { finish(session("succeeded")); await request; });
  expect(original.onSaved).not.toHaveBeenCalled(); expect(original.operations.size).toBe(1);
  renderHook(useAccioOAuth, { initialProps: original });
  expect(original.onSaved).toHaveBeenCalledOnce(); expect(original.operations.size).toBe(0); expect(apis.old.act).toHaveBeenCalledTimes(1);
});

it("does not resurrect an already consumed receipt from an older import response", async () => {
  const original = props(); const first = renderHook(useAccioOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); }); await poll();
  let finish!: (value: AccioAuthSession) => void;
  apis.old.act.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  let request!: Promise<void>; act(() => { request = first.result.current.run({ action: "import" }); }); first.unmount();
  renderHook(useAccioOAuth, { initialProps: original }); apis.old.get.mockResolvedValue(session("succeeded")); await poll();
  await act(async () => { finish(session("importing")); await request; });
  expect(original.operations.size).toBe(0); expect(original.onSaved).toHaveBeenCalledOnce();
});

it("bounds sequential poll failures and resumes GET only", async () => {
  const { result } = renderHook(useAccioOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  apis.old.get.mockRejectedValue(new Error("offline")); await poll(13500);
  expect(apis.old.get).toHaveBeenCalledTimes(5); expect(result.current.pollStopped).toBe(true);
  act(() => result.current.resumePolling()); await poll(); expect(apis.old.get).toHaveBeenCalledTimes(6); expect(apis.old.create).toHaveBeenCalledOnce();
});

it("keeps polling serial even when a GET takes several intervals", async () => {
  const { result, unmount } = renderHook(useAccioOAuth, { initialProps: props() });
  await act(async () => { await result.current.run({ action: "create", input }); });
  let signal: AbortSignal | undefined;
  apis.old.get.mockImplementation((_id, _binding, abort) => { signal = abort; return new Promise(() => {}); });
  await poll(15000); expect(apis.old.get).toHaveBeenCalledOnce(); unmount(); expect(signal?.aborted).toBe(true);
});

it("refuses a seventeenth unresolved receipt before sending another save", async () => {
  const original = props(); const { result } = renderHook(useAccioOAuth, { initialProps: original });
  await act(async () => { await result.current.run({ action: "create", input }); }); await poll();
  for (let index = 0; index < 16; index += 1) original.operations.set(`other-${index}`, { session: session("save_unconfirmed"), uncertain: true });
  await act(async () => { await result.current.run({ action: "import" }); });
  expect(apis.old.act).not.toHaveBeenCalled(); expect(result.current.error).toContain("上限"); expect(original.operations.size).toBe(16);
});
