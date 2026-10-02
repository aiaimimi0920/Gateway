import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { pushAppToast } from "../../components/AppToast";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useCredentialPoolActions } from "./useCredentialPoolActions";

vi.mock("../../components/AppToast", () => ({ pushAppToast: vi.fn() }));
beforeEach(() => vi.clearAllMocks());

function deferred() {
  let resolve!: (response: { purgedCount: number }) => void;
  let reject!: (cause: Error) => void;
  const promise = new Promise<{ purgedCount: number }>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function setup() {
  const api = createConsoleApi();
  const options: Parameters<typeof useCredentialPoolActions>[0] = {
    api, managementToken: "token", draftDirty: false,
    credentialPoolAutomationByProvider: new Map([["provider", { driverConfigured: true }]]),
    credentialRefillByProvider: new Map([["provider", { userRequestEnabled: true }]]),
    setCredentialPoolAutomation: vi.fn(), refresh: vi.fn(async () => {}),
    setError: vi.fn(), t: (_zh, en) => en,
  };
  return { api, options, ...renderHook(props => useCredentialPoolActions(props), { initialProps: options }) };
}

it.each(["token", "api", "unmount", "restore"] as const)("ignores late purge completion after %s changes", async (change) => {
  const h = setup();
  const pending = deferred();
  vi.mocked(h.api.purgeCredentialArchive).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  act(() => { request = h.result.current.handlePurgeCredentialArchive("old"); });
  if (change === "unmount") h.unmount();
  else h.rerender({ ...h.options, ...(change !== "api" ? { managementToken: "new" } : { api: createConsoleApi() }) });
  if (change === "restore") h.rerender(h.options);
  await act(async () => { pending.resolve({ purgedCount: 1 }); await request; });
  expect(pushAppToast).not.toHaveBeenCalled();
  expect(h.options.refresh).not.toHaveBeenCalled();
});

it("ignores late errors after unmount", async () => {
  const h = setup();
  const pending = deferred();
  vi.mocked(h.api.purgeCredentialArchive).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  act(() => { request = h.result.current.handlePurgeCredentialArchive("old"); });
  h.unmount();
  vi.mocked(h.options.setError).mockClear();
  await act(async () => { pending.reject(new Error("late error")); await request; });
  expect(h.options.setError).not.toHaveBeenCalled();
  expect(pushAppToast).not.toHaveBeenCalled();
});

it("preserves a newer busy marker when an earlier purge finishes", async () => {
  const h = setup();
  const old = deferred();
  const current = deferred();
  vi.mocked(h.api.purgeCredentialArchive).mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => { first = h.result.current.handlePurgeCredentialArchive("old"); });
  act(() => { second = h.result.current.handlePurgeCredentialArchive("current"); });
  await act(async () => { old.resolve({ purgedCount: 1 }); await first; });
  expect(h.result.current.credentialArchivePurgeBusy).toBe("current");
  expect(h.options.refresh).not.toHaveBeenCalled();
  await act(async () => { current.resolve({ purgedCount: 2 }); await second; });
  expect(h.result.current.credentialArchivePurgeBusy).toBeNull();
  expect(h.options.refresh).toHaveBeenCalledOnce();
});

it.each(["token", "unmount"] as const)("does not execute a retained callback after %s changes", async (change) => {
  const h = setup();
  const purge = h.result.current.handlePurgeCredentialArchive;
  if (change === "unmount") h.unmount();
  else h.rerender({ ...h.options, managementToken: "new" });
  await act(() => purge("old"));
  expect(h.api.purgeCredentialArchive).not.toHaveBeenCalled();
});

it.each(["prune", "refill"] as const)("ignores late %s errors after identity changes", async (kind) => {
  const h = setup();
  let reject!: (cause: Error) => void;
  const promise = new Promise<never>((_yes, no) => { reject = no; });
  const method = kind === "prune" ? h.api.pruneCredentialPool : h.api.requestCredentialRefill;
  vi.mocked(method).mockReturnValueOnce(promise);
  let request!: Promise<void>;
  act(() => { request = kind === "prune"
    ? h.result.current.handlePruneCredentialPool("provider")
    : h.result.current.handleRequestCredentialRefill("provider"); });
  expect(method).toHaveBeenCalledWith("token", "provider");
  h.rerender({ ...h.options, managementToken: "new" });
  vi.mocked(h.options.setError).mockClear();
  await act(async () => { reject(new Error("late error")); await request; });
  expect(h.options.setError).not.toHaveBeenCalled();
  expect(h.options.refresh).not.toHaveBeenCalled();
  expect(h.options.setCredentialPoolAutomation).not.toHaveBeenCalled();
  expect(pushAppToast).not.toHaveBeenCalled();
});

it("keeps refill busy when a concurrent purge completes", async () => {
  const h = setup();
  const purge = deferred();
  let reject!: (cause: Error) => void;
  vi.mocked(h.api.purgeCredentialArchive).mockReturnValueOnce(purge.promise);
  vi.mocked(h.api.requestCredentialRefill).mockReturnValueOnce(new Promise<never>((_yes, no) => { reject = no; }));
  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => { first = h.result.current.handlePurgeCredentialArchive("provider"); });
  act(() => { second = h.result.current.handleRequestCredentialRefill("provider"); });
  await act(async () => { purge.resolve({ purgedCount: 1 }); await first; });
  expect(h.result.current.credentialArchivePurgeBusy).toBeNull();
  expect(h.result.current.credentialRefillBusy).toBe("provider");
  expect(h.options.refresh).toHaveBeenCalledOnce();
  await act(async () => { reject(new Error("refill unavailable")); await second; });
  expect(h.result.current.credentialRefillBusy).toBeNull();
  expect(h.options.setError).toHaveBeenLastCalledWith("refill unavailable");
});


it("blocks purge while the displayed archive path is an uncommitted draft", async () => {
  const h = setup();
  const retained = h.result.current.handlePurgeCredentialArchive;
  h.rerender({ ...h.options, draftDirty: true });
  await act(() => h.result.current.handlePurgeCredentialArchive("provider"));
  await act(() => retained("provider"));
  expect(h.api.purgeCredentialArchive).not.toHaveBeenCalled();
  expect(pushAppToast).toHaveBeenCalledWith("warning", expect.stringContaining("Save the current route draft"));
});

it("refreshes after purge failure because the durable barrier may have committed", async () => {
  const h = setup();
  vi.mocked(h.api.purgeCredentialArchive).mockRejectedValueOnce(new Error("purge barrier remains committed"));
  await act(() => h.result.current.handlePurgeCredentialArchive("provider"));
  expect(h.options.refresh).toHaveBeenCalledOnce();
  expect(h.options.setError).toHaveBeenLastCalledWith("purge barrier remains committed");
});
