import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteConfigResponse } from "../../api/contracts";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useConsoleRouteData } from "./useConsoleRouteData";

function deferred() {
  let resolve!: (value: ConsoleRouteConfigResponse) => void;
  const promise = new Promise<ConsoleRouteConfigResponse>((yes) => { resolve = yes; });
  return { promise, resolve };
}

function setup() {
  const api = createConsoleApi();
  const invalidate = vi.fn();
  const hook = renderHook(({ token }: { token: string | null }) => useConsoleRouteData({
    api, managementToken: token, t: (_zh, en) => en, invalidateCredentialProbes: invalidate,
  }), { initialProps: { token: "initial" as string | null } });
  return { api, invalidate, ...hook };
}

it("does not publish an old route snapshot after logout", async () => {
  const h = setup();
  const snapshot = await h.api.getRouteConfig("fixture");
  const pending = deferred();
  vi.mocked(h.api.getRouteConfig).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  await act(async () => { request = h.result.current.refresh(); await Promise.resolve(); });
  h.rerender({ token: null });
  await act(() => h.result.current.refresh());
  await act(async () => { pending.resolve(snapshot); await request; });
  expect(h.result.current.routeConfig).toBeNull();
  expect(h.result.current.error).toBe("Gateway management token is unavailable.");
  expect(h.result.current.busy).toBe(false);
});

it("starts a fresh request when the same token is restored before the old request settles", async () => {
  const h = setup();
  const snapshot = await h.api.getRouteConfig("fixture");
  const old = deferred();
  vi.mocked(h.api.getRouteConfig).mockClear().mockReturnValueOnce(old.promise)
    .mockResolvedValueOnce({ routeConfig: { ...snapshot.routeConfig,
      revision: { ...snapshot.routeConfig.revision, id: "restored" } } });
  let oldRequest!: Promise<void>;
  await act(async () => { oldRequest = h.result.current.refresh(); await Promise.resolve(); });
  h.rerender({ token: null });
  h.rerender({ token: "initial" });
  let restored!: Promise<void>;
  await act(async () => { restored = h.result.current.refresh(); await Promise.resolve(); });
  expect(h.api.getRouteConfig).toHaveBeenCalledTimes(2);
  await act(async () => { old.resolve(snapshot); await Promise.all([oldRequest, restored]); });
  expect(h.result.current.routeConfig?.routeConfig.revision.id).toBe("restored");
});

it("does not start a queued refresh after unmount", async () => {
  const h = setup();
  let request!: Promise<void>;
  act(() => { request = h.result.current.refresh(); });
  h.unmount();
  await request;
  expect(h.api.getRouteConfig).not.toHaveBeenCalled();
  expect(h.invalidate).not.toHaveBeenCalled();
});

it.each([null, "different-token"])("clears published snapshots immediately on token change to %s", async (token) => {
  const h = setup();
  await act(() => h.result.current.refresh());
  expect(h.result.current.routeConfig).not.toBeNull();
  h.rerender({ token });
  for (const key of ["routeConfig", "accountGroupSummary", "providerCredentialInventory",
    "credentialPoolAutomation", "credentialRefill", "runtimePressure", "costOverview",
    "requestAuditSummary", "credentialModelStates"] as const) {
    expect(h.result.current[key]).toBeNull();
  }
  expect(h.result.current.busy).toBe(token !== null);
});
