import { afterEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createVertexAuthApi, type VertexAuthSession } from "./vertex-auth";

const binding = { providerId: "renamed-vertex", expectedRevision: "r1", credentialId: "stable" };
const input = { ...binding, clientId: "explicit-client", clientSecret: "private-client-secret",
  target: { project: "project-a", location: "global", quota_project: "quota-a" } };
const view = (): VertexAuthSession => ({ id: "login/特殊", ...binding, status: "waiting_user",
  authorizationUrl: "https://accounts.google.com/o/oauth2/v2/auth", redirectUri: "http://127.0.0.1:43567/",
  credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null });
function fixture(session: unknown = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createVertexAuthApi({ request } as GatewayApiClient, "owner") };
}
afterEach(() => vi.useRealTimers());
it("uses the independent Vertex root, snake-case target and Secret Grant only for mutations", async () => {
  const { api, request } = fixture(); await api.create(input, "grant");
  expect(request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/vertex-auth-sessions", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", body: input, signal: expect.any(AbortSignal),
  }));
  for (const action of ["complete", "cancel", "import"] as const) {
    await api.act(view().id, action, binding, "grant", "private-callback");
    expect(request).toHaveBeenLastCalledWith(expect.stringContaining(`/login%2F%E7%89%B9%E6%AE%8A/${action}`), expect.anything(), expect.objectContaining({
      body: { expectedRevision: "r1", ...(action === "complete" ? { callbackUrl: "private-callback" } : {}) }, secretGrant: "grant",
    }));
  }
  await api.get(view().id, binding); expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("secretGrant");
});
it.each([{ providerId: "other" }, { expectedRevision: "r2" }, { id: "other" }, { targetCredentialId: "other" }, { credentialId: "other" },
  { status: "succeeded", credentialId: "stable", savedRevision: null }, { status: "succeeded", credentialId: null, savedRevision: "r2" },
  { status: "fake-success" }, { redirectUri: "http://localhost:43567/" }, { redirectUri: "http://127.0.0.1:65536/" },
  { redirectUri: "https://127.0.0.1:43567/" }, { redirectUri: "http://127.0.0.1:43567/evil" }])("rejects another operation or invalid listener: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch }); await expect(api.get(view().id, binding)).rejects.toThrow();
});
it("bounds requests and propagates aborted polls without leaking timers", async () => {
  vi.useFakeTimers();
  const request: GatewayApiClient["request"] = (_path, _schema, options) => new Promise((_resolve, reject) => {
    if (options!.signal!.aborted) reject(new Error("aborted"));
    else options!.signal!.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
  });
  const api = createVertexAuthApi({ request }, "owner");
  const hung = api.create(input, "grant"); const rejected = expect(hung).rejects.toThrow("aborted");
  await vi.advanceTimersByTimeAsync(30_000); await rejected;
  const abort = new AbortController(); abort.abort(); await expect(api.get(view().id, binding, abort.signal)).rejects.toThrow("aborted");
  expect(vi.getTimerCount()).toBe(0);
});
