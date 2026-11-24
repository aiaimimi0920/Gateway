import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useConsoleDraftPersistence } from "./useConsoleDraftPersistence";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { deferred } from "./BrowserConsoleApp.render-fixture";

afterEach(() => vi.useRealTimers());
it("retries a secret edit once after reauthorization, without looping while permission is missing", async () => {
  vi.useFakeTimers();
  const api = createConsoleApi();
  vi.mocked(api.commitRouteConfig).mockRejectedValueOnce(new Error("expired permission"));
  const document = { providers: [], model_routes: [], aliases: {} };
  const secretPatches = [{ path: "/providers/0/credentials/0/api_key", operation: "replace" as const, value: "synthetic" }];
  const options: Parameters<typeof useConsoleDraftPersistence>[0] = {
    routeConfig: null,
    managementToken: "test", secretGrant: "expired", editorLocked: false,
    selectedGroupIdInvalid: false, selectedGroupBillingInvalid: false, editorText: JSON.stringify(document),
    commitMessage: "", secretPatches, draftDirty: true, draftDocumentState: { document, invalid: false },
    incompleteAccountGroupDraftCount: 0, parseDraft: () => ({ document, secretPatches, expectedRevision: "r1" }),
    setRouteConfig: vi.fn(), refresh: vi.fn().mockResolvedValue(undefined), setError: vi.fn(),
    consoleActionGenerationRef: { current: 1 },
    beginConsoleActionRequest: () => ({ api, generation: 1, managementToken: "test", secretGrant: "test-grant", secretGrantEpoch: 0 }),
    isConsoleActionRequestCurrent: () => true, isConsoleActionRecoveryCurrent: () => true, setActionBusy: vi.fn(),
    handleSecretAccessRequiredError: () => "recovered", t: zh => zh,
  };
  const h = renderHook(props => useConsoleDraftPersistence(props), { initialProps: options });
  await act(() => vi.advanceTimersByTimeAsync(1300));
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(1);
  h.rerender({ ...options, secretGrant: null });
  await act(() => vi.advanceTimersByTimeAsync(10000));
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(1);
  h.rerender({ ...options, secretGrant: "renewed" });
  await act(() => vi.advanceTimersByTimeAsync(1300));
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(2);
  await act(() => vi.advanceTimersByTimeAsync(10000));
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(2);
});

it("does not let a stale in-flight save block or unlock a restored session's save", async () => {
  const api = createConsoleApi();
  type Result = Awaited<ReturnType<typeof api.commitRouteConfig>>;
  const response = await api.commitRouteConfig("test", { expectedRevision: "r1", document: { providers: [], model_routes: [], aliases: {} }, secretPatches: [] });
  const old = deferred<Result>(), active = deferred<Result>();
  vi.mocked(api.commitRouteConfig).mockClear().mockReturnValueOnce(old.promise).mockReturnValueOnce(active.promise);
  const document = { providers: [], model_routes: [], aliases: {} }, generation = { current: 1 };
  const options: Parameters<typeof useConsoleDraftPersistence>[0] = {
    routeConfig: null, managementToken: "test", secretGrant: null, editorLocked: true,
    selectedGroupIdInvalid: false, selectedGroupBillingInvalid: false, editorText: JSON.stringify(document),
    commitMessage: "", secretPatches: [], draftDirty: true, draftDocumentState: { document, invalid: false },
    incompleteAccountGroupDraftCount: 0, parseDraft: () => ({ document, secretPatches: [], expectedRevision: "r1" }),
    setRouteConfig: vi.fn(), refresh: vi.fn().mockResolvedValue(undefined), setError: vi.fn(), consoleActionGenerationRef: generation,
    beginConsoleActionRequest: () => ({ api, generation: generation.current, managementToken: "test", secretGrant: null, secretGrantEpoch: 0 }),
    isConsoleActionRequestCurrent: action => action.generation === generation.current,
    isConsoleActionRecoveryCurrent: () => false, setActionBusy: vi.fn(), handleSecretAccessRequiredError: () => "not-required", t: zh => zh,
  };
  const h = renderHook(() => useConsoleDraftPersistence(options));
  let first!: Promise<void>, second!: Promise<void>;
  act(() => { first = h.result.current.retrySave({ silent: true }); });
  generation.current = 2;
  act(() => { second = h.result.current.retrySave({ silent: true }); });
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(2);
  await act(async () => { old.resolve(response); await first; });
  await act(() => h.result.current.retrySave({ silent: true }));
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(2);
  expect(options.setRouteConfig).not.toHaveBeenCalled();
  expect(options.setActionBusy).not.toHaveBeenCalledWith(null);
  await act(async () => { active.resolve(response); await second; });
  expect(options.setRouteConfig).toHaveBeenCalledTimes(1);
  expect(options.setActionBusy).toHaveBeenLastCalledWith(null);
});
