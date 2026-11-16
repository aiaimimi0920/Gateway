import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useConsoleDraftPersistence } from "./useConsoleDraftPersistence";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";

afterEach(() => vi.useRealTimers());
it("retries a secret edit once after reauthorization, without looping while permission is missing", async () => {
  vi.useFakeTimers();
  const api = createConsoleApi();
  vi.mocked(api.commitRouteConfig).mockRejectedValueOnce(new Error("expired permission"));
  const document = { providers: [], model_routes: [], aliases: {} };
  const secretPatches = [{ path: "/providers/0/credentials/0/api_key", operation: "replace" as const, value: "synthetic" }];
  const options: Parameters<typeof useConsoleDraftPersistence>[0] = {
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
