import { act, renderHook } from "@testing-library/react";
import { createElement, StrictMode, type ReactNode } from "react";
import { expect, it, vi } from "vitest";
import type { RouteManagedAccount } from "./routeAccountCatalog";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useConsoleProbeActions } from "./useConsoleProbeActions";

const account: RouteManagedAccount = {
  id: "account", displayName: "Account", vendorKey: "custom", vendorName: "Custom",
  providerId: "provider", providerLabel: "Provider", providerPreset: null,
  baseUrl: null, hostLabel: null, mode: "credential", enabled: true,
  supportedModels: [], groupIds: [], groupNames: [],
};

function setup(strict = false) {
  const api = createConsoleApi();
  const options: Parameters<typeof useConsoleProbeActions>[0] = {
    api, managementToken: "token",
    session: { secretGrant: { grant: "grant", expiresAt: "2099-01-01T00:00:00Z" } },
    draftDirty: false, draftMatchesActiveRevision: true,
    credentialProbeGenerationRef: { current: 0 }, providerProbeGenerationRef: { current: 0 },
    currentApiRef: { current: api }, currentManagementTokenRef: { current: "token" },
    currentSecretGrantEpochRef: { current: 0 }, currentSecretGrantRef: { current: "grant" },
    handleSecretAccessRequiredError: vi.fn(() => "not-required" as const),
    setCredentialProbeBusy: vi.fn(), setCredentialProbeResults: vi.fn(),
    setProviderProbeBusy: vi.fn(), setProviderProbeError: vi.fn(),
    setProviderProbeResponse: vi.fn(), setSecretDialogOpen: vi.fn(), setError: vi.fn(),
    t: (_zh, en) => en,
  };
  const hook = renderHook(props => {
    props.currentApiRef.current = props.api;
    props.currentManagementTokenRef.current = props.managementToken;
    return useConsoleProbeActions(props);
  }, {
    initialProps: options,
    wrapper: strict ? ({ children }: { children: ReactNode }) => createElement(StrictMode, null, children) : undefined,
  });
  for (const setter of [options.setCredentialProbeBusy, options.setCredentialProbeResults,
    options.setProviderProbeBusy, options.setProviderProbeError, options.setProviderProbeResponse]) {
    vi.mocked(setter).mockClear();
  }
  return { api, options, ...hook };
}

it("does not recover secret access or publish errors after unmount", async () => {
  const h = setup();
  let reject!: (cause: Error) => void;
  vi.mocked(h.api.probeCredential).mockReturnValueOnce(new Promise<never>((_yes, no) => { reject = no; }));
  let request!: Promise<void>;
  act(() => { request = h.result.current.handleCredentialProbe(account); });
  h.unmount();
  vi.mocked(h.options.setCredentialProbeBusy).mockClear();
  await act(async () => { reject(new Error("late error")); await request; });
  expect(h.options.handleSecretAccessRequiredError).not.toHaveBeenCalled();
  expect(h.options.setCredentialProbeResults).not.toHaveBeenCalled();
  expect(h.options.setCredentialProbeBusy).not.toHaveBeenCalled();
});

it("does not publish a successful probe after unmount", async () => {
  const h = setup();
  const response = await h.api.probeCredential("token", "grant", "account");
  let resolve!: (value: typeof response) => void;
  vi.mocked(h.api.probeCredential).mockReturnValueOnce(new Promise(yes => { resolve = yes; }));
  let request!: Promise<void>;
  act(() => { request = h.result.current.handleCredentialProbe(account); });
  h.unmount();
  vi.mocked(h.options.setCredentialProbeBusy).mockClear();
  await act(async () => { resolve(response); await request; });
  expect(h.options.setCredentialProbeResults).not.toHaveBeenCalled();
  expect(h.options.setCredentialProbeBusy).not.toHaveBeenCalled();
});

it("does not start a request from a retained callback after unmount", async () => {
  const h = setup();
  const probe = h.result.current.handleCredentialProbe;
  h.unmount();
  await act(() => probe(account));
  expect(h.api.probeCredential).not.toHaveBeenCalled();
  expect(h.options.setCredentialProbeBusy).not.toHaveBeenCalled();
});

it.each(["success", "error"] as const)("ignores provider %s after unmount", async (outcome) => {
  const h = setup();
  const response = await h.api.probeProvider("token", "grant", "provider");
  let resolve!: (value: typeof response) => void;
  let reject!: (cause: Error) => void;
  vi.mocked(h.api.probeProvider).mockReturnValueOnce(new Promise((yes, no) => { resolve = yes; reject = no; }));
  let request!: Promise<void>;
  act(() => { request = h.result.current.handleProviderProbe({ providerId: "provider" }); });
  h.unmount();
  vi.mocked(h.options.setProviderProbeResponse).mockClear();
  vi.mocked(h.options.setProviderProbeError).mockClear();
  vi.mocked(h.options.setProviderProbeBusy).mockClear();
  await act(async () => {
    if (outcome === "success") resolve(response); else reject(new Error("late error"));
    await request;
  });
  expect(h.options.handleSecretAccessRequiredError).not.toHaveBeenCalled();
  expect(h.options.setProviderProbeResponse).not.toHaveBeenCalled();
  expect(h.options.setProviderProbeError).not.toHaveBeenCalled();
  expect(h.options.setProviderProbeBusy).not.toHaveBeenCalled();
});

it("does not start a provider request from a retained callback after unmount", async () => {
  const h = setup();
  const probe = h.result.current.handleProviderProbe;
  h.unmount();
  await act(() => probe({ providerId: "provider" }));
  expect(h.api.probeProvider).not.toHaveBeenCalled();
  expect(h.options.setProviderProbeBusy).not.toHaveBeenCalled();
});

it("allows both probes after StrictMode effect replay", async () => {
  const h = setup(true);
  await act(() => h.result.current.handleCredentialProbe(account));
  await act(() => h.result.current.handleProviderProbe({ providerId: "provider" }));
  expect(h.options.setCredentialProbeResults).toHaveBeenCalledOnce();
  expect(h.options.setProviderProbeResponse).toHaveBeenLastCalledWith(
    await vi.mocked(h.api.probeProvider).mock.results[0].value,
  );
});

it.each(["token", "api"] as const)("does not revive a provider result after %s restoration", async (change) => {
  const h = setup();
  const response = await h.api.probeProvider("token", "grant", "provider");
  let resolve!: (value: typeof response) => void;
  vi.mocked(h.api.probeProvider).mockReturnValueOnce(new Promise(yes => { resolve = yes; }));
  let request!: Promise<void>;
  act(() => { request = h.result.current.handleProviderProbe({ providerId: "provider" }); });
  h.rerender({ ...h.options, ...(change === "token" ? { managementToken: "new" } : { api: createConsoleApi() }) });
  h.rerender(h.options);
  vi.mocked(h.options.setProviderProbeResponse).mockClear();
  vi.mocked(h.options.setProviderProbeBusy).mockClear();
  await act(async () => { resolve(response); await request; });
  expect(h.options.setProviderProbeResponse).not.toHaveBeenCalled();
  expect(h.options.setProviderProbeBusy).not.toHaveBeenCalled();
});

it.each(["token", "api"] as const)("does not recover credential errors after %s restoration", async (change) => {
  const h = setup();
  let reject!: (cause: Error) => void;
  vi.mocked(h.api.probeCredential).mockReturnValueOnce(new Promise<never>((_yes, no) => { reject = no; }));
  let request!: Promise<void>;
  act(() => { request = h.result.current.handleCredentialProbe(account); });
  h.rerender({ ...h.options, ...(change === "token" ? { managementToken: "new" } : { api: createConsoleApi() }) });
  h.rerender(h.options);
  vi.mocked(h.options.setCredentialProbeResults).mockClear();
  await act(async () => { reject(new Error("late error")); await request; });
  expect(h.options.setCredentialProbeResults).not.toHaveBeenCalled();
  expect(h.options.handleSecretAccessRequiredError).not.toHaveBeenCalled();
});

it.each(["credential", "provider"] as const)("does not start a retained %s callback under another identity", async (kind) => {
  const h = setup();
  const credential = h.result.current.handleCredentialProbe;
  const provider = h.result.current.handleProviderProbe;
  h.rerender({ ...h.options, managementToken: "new" });
  await act(async () => {
    if (kind === "credential") await credential(account);
    else await provider({ providerId: "provider" });
  });
  expect(h.api.probeCredential).not.toHaveBeenCalled();
  expect(h.api.probeProvider).not.toHaveBeenCalled();
  expect(h.options.setProviderProbeBusy).toHaveBeenLastCalledWith(false);
  expect(h.options.setProviderProbeResponse).toHaveBeenLastCalledWith(null);
  expect(h.options.setCredentialProbeBusy).toHaveBeenLastCalledWith(null);
});
