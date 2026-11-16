import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import type { DimAuthInput, DimAuthSession } from "../../api/console/dim-auth";
import { GatewayApiError } from "../../api/errors";
import { useDimOAuth, type DimOAuthProps } from "./useDimOAuth";

const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/dim-auth", () => ({ createDimAuthApi: () => api }));
const session = (status: DimAuthSession["status"]): DimAuthSession => ({ id: "login", providerId: "dim", expectedRevision: "r1", site: "china", status,
  authorizationUrl: "https://dimagent.cn/oauth/authorize?state=synthetic", redirectUri: "http://127.0.0.1:43567/oauth/callback",
  identityAssurance: status === "ready" ? "user_info_endpoint" : null, credentialId: status === "succeeded" ? "stable" : null,
  targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props = (): DimOAuthProps => ({ client: {} as GatewayApiClient, managementToken: "owner", secretGrant: "grant", providerId: "dim", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() });
const input: DimAuthInput = { providerId: "dim", expectedRevision: "r1", site: "china", clientId: "permitted-gateway",
  scopes: ["openid", "profile", "email"], redirectUri: "http://127.0.0.1:43567/oauth/callback" };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user")); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => vi.useRealTimers());
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };

it("releases an unavailable unsaved session and requires an explicit new login", async () => {
  api.get.mockRejectedValue(new GatewayApiError("dim_login_unavailable", 400, "dim_login_rejected"));
  const original = props(); const hook = renderHook(useDimOAuth, { initialProps: original });
  await act(async () => { await hook.result.current.run({ action: "create", input }); });
  await poll();
  expect(hook.result.current.session).toBeNull(); expect(hook.result.current.active).toBe(false);
  expect(hook.result.current.error).toContain("会话已不可用");
  expect(api.get).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce();
  expect(original.onSaved).not.toHaveBeenCalled(); expect(original.operations.size).toBe(0);
  await act(async () => { await hook.result.current.run({ action: "create", input }); });
  expect(api.create).toHaveBeenCalledTimes(2);
});

it("retains the duplicate-save fence when a lost save receipt is reclaimed", async () => {
  api.create.mockResolvedValueOnce(session("ready"));
  api.act.mockRejectedValueOnce(new Error("response lost"));
  const original = props(); const first = renderHook(useDimOAuth, { initialProps: original });
  await act(async () => { await first.result.current.run({ action: "create", input }); });
  await act(async () => { await first.result.current.run({ action: "import" }); });
  api.get.mockRejectedValue(new GatewayApiError("dim_login_unavailable", 400, "dim_login_rejected"));
  await poll();
  expect(first.result.current.pollStopped).toBe(true); expect(first.result.current.uncertain).toBe(true);
  expect(first.result.current.error).toContain("保存回执已不可用");
  first.unmount();
  const reopened = renderHook(useDimOAuth, { initialProps: original });
  await act(async () => {
    await reopened.result.current.run({ action: "create", input });
    await reopened.result.current.run({ action: "import" });
  });
  expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledOnce();
  expect(original.onSaved).not.toHaveBeenCalled(); expect(original.operations.size).toBe(1);
});

it.each([
  ["dim_login_capacity", "dim_login_rejected"],
  ["dim_login_unavailable", "console_revision_conflict"],
])("does not clear a session for a different backend failure: %s / %s", async (message, code) => {
  api.get.mockRejectedValue(new GatewayApiError(message, 400, code));
  const hook = renderHook(useDimOAuth, { initialProps: props() });
  await act(async () => { await hook.result.current.run({ action: "create", input }); });
  await poll();
  expect(hook.result.current.session?.id).toBe("login"); expect(hook.result.current.active).toBe(true);
  expect(hook.result.current.pollStopped).toBe(false); expect(api.create).toHaveBeenCalledOnce();
});

it("does not auto-save a ready receipt and follows one admitted save across a revision change", async () => {
  const original = props(); const hook = renderHook((p) => useDimOAuth(p), { initialProps: original });
  await act(async () => { await hook.result.current.run({ action: "create", input }); }); await poll();
  expect(hook.result.current.session?.identityAssurance).toBe("user_info_endpoint"); expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("importing")); await act(async () => { await hook.result.current.run({ action: "import" }); });
  hook.rerender({ ...original, expectedRevision: "r2", disabled: true }); api.get.mockResolvedValue(session("succeeded")); await poll();
  expect(original.onSaved).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledOnce();
});
it("does not rebase a create waiting for a renewed Secret Grant", async () => {
  const original: DimOAuthProps = { ...props(), secretGrant: null }; const hook = renderHook((p: DimOAuthProps) => useDimOAuth(p), { initialProps: original });
  await act(async () => { await hook.result.current.run({ action: "create", input }); });
  expect(original.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(api.create).not.toHaveBeenCalled();
  hook.rerender({ ...original, expectedRevision: "r2", secretGrant: "renewed" }); await act(async () => {});
  expect(api.create).not.toHaveBeenCalled(); expect(original.operations.size).toBe(0);
});
it("recovers only the captured callback operation and keeps callback material out of receipts", async () => {
  const original = props(); const hook = renderHook((p) => useDimOAuth(p), { initialProps: original });
  await act(async () => { await hook.result.current.run({ action: "create", input }); }); hook.rerender({ ...original, secretGrant: null });
  const callbackUrl = `${input.redirectUri}?code=private-callback&state=synthetic`;
  await act(async () => { await hook.result.current.run({ action: "complete", callbackUrl }); }); expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("ready")); hook.rerender({ ...original, secretGrant: "renewed" }); await act(async () => {});
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "complete", expect.objectContaining({ expectedRevision: "r1" }), "renewed", callbackUrl);
  expect(JSON.stringify([...original.operations.values()])).not.toContain("private-callback");
});
it("retains an unknown save across reopen and never repeats login or import", async () => {
  const original = props(); const hook = renderHook(() => useDimOAuth(original));
  await act(async () => { await hook.result.current.run({ action: "create", input }); }); await poll();
  api.act.mockRejectedValueOnce(new Error("lost response")); await act(async () => { await hook.result.current.run({ action: "import" }); });
  hook.unmount(); const reopened = renderHook(() => useDimOAuth(original));
  await act(async () => { await reopened.result.current.run({ action: "create", input }); await reopened.result.current.run({ action: "import" }); });
  expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledOnce();
  api.get.mockResolvedValue(session("succeeded")); await poll(); expect(original.onSaved).toHaveBeenCalledOnce();
});
it("distinguishes a known pre-save identity rejection from an unknown accepted save", async () => {
  const original = props(); const hook = renderHook(() => useDimOAuth(original));
  await act(async () => { await hook.result.current.run({ action: "create", input }); }); await poll();
  api.act.mockRejectedValueOnce(new GatewayApiError("identity mismatch", 400, "dim_login_identity_mismatch"));
  await act(async () => { await hook.result.current.run({ action: "import" }); });
  expect(hook.result.current.uncertain).toBe(false); expect(original.operations.size).toBe(0);
});
