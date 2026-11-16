import { afterEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createGrokAuthApi, type GrokAuthSession } from "./grok-auth";

const binding = { providerId: "custom-build", expectedRevision: "r1", credentialId: "stable-account" };
const input = { ...binding, clientId: "permitted-client", scopes: ["openid"], redirectUri: "http://127.0.0.1:34567/callback" };
const view = (): GrokAuthSession => ({ id: "login/特殊", ...binding, status: "waiting_user", authorizationUrl: "https://auth.x.ai/oauth2/authorize",
  identityVerified: false, credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null });
function fixture(session = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createGrokAuthApi({ request } as GatewayApiClient, "owner-token") };
}
afterEach(() => vi.useRealTimers());

it("preserves the server callback mode while retaining compatibility with manual sessions", async () => {
  const { api, request } = fixture({ ...view(), automaticCallback: true });
  expect((await api.create({ ...input, authorizationFlow: "loopback" }, "grant")).automaticCallback).toBe(true);
  expect(request).toHaveBeenLastCalledWith(expect.anything(), expect.anything(), expect.objectContaining({
    body: { ...input, authorizationFlow: "loopback" }, secretGrant: "grant",
  }));
  expect((await fixture().api.create(input, "grant")).automaticCallback).toBeUndefined();
});

it("binds every mutation to the owner, secret grant and original revision without leaking callback into other actions", async () => {
  const { api, request } = fixture();
  await api.create(input, "grant");
  expect(request).toHaveBeenLastCalledWith(expect.stringMatching(/grok-auth-sessions$/), expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner-token", secretGrant: "grant", body: input, signal: expect.any(AbortSignal),
  }));
  for (const action of ["complete", "cancel", "import"] as const) {
    await api.act(view().id, action, binding, "grant", "private-callback");
    expect(request).toHaveBeenLastCalledWith(expect.stringContaining(`/login%2F%E7%89%B9%E6%AE%8A/${action}`), expect.anything(), expect.objectContaining({
      method: "POST", managementToken: "owner-token", secretGrant: "grant",
      body: { expectedRevision: "r1", ...(action === "complete" ? { callbackUrl: "private-callback" } : {}) },
    }));
  }
  await api.get(view().id, binding);
  const options = request.mock.calls.at(-1)![2] as Record<string, unknown>;
  expect(options.managementToken).toBe("owner-token");
  expect(options.secretGrant).toBeUndefined();
});

it.each([
  { providerId: "different" }, { expectedRevision: "r2" }, { id: "different" },
  { targetCredentialId: "different" }, { credentialId: "different" },
  { status: "succeeded", credentialId: "stable-account", savedRevision: null },
  { status: "succeeded", credentialId: null, savedRevision: "r2" },
])("rejects a mismatched or incomplete response: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch } as GrokAuthSession);
  await expect(api.get(view().id, binding)).rejects.toThrow("does not match");
});

it("validates the response schema instead of accepting an unknown terminal status", async () => {
  const { api } = fixture({ ...view(), status: "fake-success" } as unknown as GrokAuthSession);
  await expect(api.create(input, "grant")).rejects.toThrow();
});

it("aborts a hung request after the bounded budget and removes its timer", async () => {
  vi.useFakeTimers();
  const request: GatewayApiClient["request"] = (_path, _schema, options) => new Promise((_resolve, reject) => {
    options!.signal!.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
  });
  const result = createGrokAuthApi({ request }, "token").create(input, "grant");
  const rejected = expect(result).rejects.toThrow("aborted");
  await vi.advanceTimersByTimeAsync(30_000); await rejected;
  expect(vi.getTimerCount()).toBe(0);
});

it("propagates poll cancellation and releases the external abort listener", async () => {
  const { api } = fixture();
  const abort = new AbortController();
  const remove = vi.spyOn(abort.signal, "removeEventListener");
  await api.get(view().id, binding, abort.signal);
  expect(remove).toHaveBeenCalledWith("abort", expect.any(Function));
});
