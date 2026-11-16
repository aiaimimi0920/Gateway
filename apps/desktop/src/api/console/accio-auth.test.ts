import { afterEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createAccioAuthApi, type AccioAuthSession } from "./accio-auth";

const binding = { providerId: "renamed-accio", expectedRevision: "r1", credentialId: "stable-account" };
const input = { ...binding, clientId: "permitted-client", loginSite: "global" as const, callbackPort: 34567, utdid: "explicit-device" };
const view = (): AccioAuthSession => ({ id: "login/特殊", providerId: binding.providerId, expectedRevision: "r1", status: "waiting_user",
  authorizationUrl: "https://zh.accio.com/login", credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null });
function fixture(session: unknown = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createAccioAuthApi({ request } as GatewayApiClient, "owner-token") };
}
afterEach(() => vi.useRealTimers());

it("uses the Accio root and explicit inputs without requiring a Grok identity field", async () => {
  const { api, request } = fixture();
  expect(await api.create(input, "grant")).toEqual(view());
  expect(request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/accio-auth-sessions", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner-token", secretGrant: "grant", body: input, signal: expect.any(AbortSignal),
  }));
  for (const action of ["complete", "cancel", "import"] as const) {
    await api.act(view().id, action, binding, "grant", "private-callback");
    expect(request).toHaveBeenLastCalledWith(expect.stringContaining(`/login%2F%E7%89%B9%E6%AE%8A/${action}`), expect.anything(), expect.objectContaining({
      body: { expectedRevision: "r1", ...(action === "complete" ? { callbackUrl: "private-callback" } : {}) },
      method: "POST", managementToken: "owner-token", secretGrant: "grant",
    }));
  }
  await api.get(view().id, binding);
  expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("secretGrant");
});

it.each([
  { providerId: "different" }, { expectedRevision: "r2" }, { id: "different" },
  { targetCredentialId: "different" }, { credentialId: "different" },
  { status: "succeeded", credentialId: "stable-account", savedRevision: null },
  { status: "succeeded", credentialId: null, savedRevision: "r2" },
  { status: "claimed" }, { status: "fake-success" },
])("rejects a mismatched or non-Accio session: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch });
  await expect(api.get(view().id, binding)).rejects.toThrow();
});

it("bounds requests and propagates already aborted poll signals without leaving a timer", async () => {
  vi.useFakeTimers();
  const request: GatewayApiClient["request"] = (_path, _schema, options) => new Promise((_resolve, reject) => {
    if (options!.signal!.aborted) reject(new Error("aborted"));
    else options!.signal!.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
  });
  const api = createAccioAuthApi({ request }, "token");
  const hung = api.create(input, "grant"); const rejected = expect(hung).rejects.toThrow("aborted");
  await vi.advanceTimersByTimeAsync(30_000); await rejected;
  const abort = new AbortController(); abort.abort();
  const remove = vi.spyOn(abort.signal, "removeEventListener");
  await expect(api.get(view().id, binding, abort.signal)).rejects.toThrow("aborted");
  expect(remove).toHaveBeenCalledWith("abort", expect.any(Function)); expect(vi.getTimerCount()).toBe(0);
});
