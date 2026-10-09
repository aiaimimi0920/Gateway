import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ConsoleProviderProbeResponse } from "../../api/contracts";
import { TEST_RESULT_REFRESH_MS, useTestPlanResults } from "./useTestPlanResults";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";

const response = (id: string): ConsoleProviderProbeResponse => ({ result: { providerId: "p", status: "passed", message: "round", checkedAt: "now",
  totalCount: 1, passedCount: 1, failedCount: 0, unsupportedCount: 0, results: [{ providerId: "p", credentialId: "one", checkedAt: "now",
    status: "passed", message: "round", probePoint: "matrix", assessment: { planId: id, mode: "manual", policySource: `plan:${id}`, models: [], cases: [] } }] } });

it("expansion cannot change the summary sample and repeated expansion replaces rather than grows the cache", async () => {
  const loader = vi.fn().mockImplementation(async (_provider, _scope, query) => response(query.planId ?? "first"));
  const { result } = renderHook(() => useTestPlanResults(["p"], loader, true));
  await waitFor(() => expect(result.current.loading).toBe(false));
  const row = { key: "second", id: "second", name: "Second", providerId: "p", legacy: false, policy: defaultTestPolicy(),
    scopes: [{ kind: "pool" as const, providerId: "p" }] };
  for (let index = 0; index < 3; index++) await act(() => result.current.expand(row));
  expect(result.current.summaryResults.map((row) => row.assessment?.planId)).toEqual(["first"]);
  expect(result.current.results.map((row) => row.assessment?.planId)).toEqual(["second", "first"]);
  expect(result.current.responses).toHaveLength(2);
});

afterEach(() => vi.useRealTimers());

it("refreshes summaries and expanded plans automatically without overlapping pending reads", async () => {
  vi.useFakeTimers();
  let complete!: (response: ConsoleProviderProbeResponse) => void;
  const loader = vi.fn().mockReturnValueOnce(new Promise<ConsoleProviderProbeResponse>((resolve) => { complete = resolve; }))
    .mockImplementation(async (_provider, _scope, query) => response(query.planId ?? "latest"));
  const hook = renderHook(() => useTestPlanResults(["p"], loader, true));
  await act(async () => { await vi.advanceTimersByTimeAsync(TEST_RESULT_REFRESH_MS * 3); });
  expect(loader).toHaveBeenCalledOnce();
  await act(async () => { complete(response("first")); });
  const row = { key: "second", id: "second", name: "Second", providerId: "p", legacy: false, policy: defaultTestPolicy(),
    scopes: [{ kind: "pool" as const, providerId: "p" }] };
  await act(() => hook.result.current.expand(row));
  await act(async () => { await vi.advanceTimersByTimeAsync(TEST_RESULT_REFRESH_MS); });
  expect(loader).toHaveBeenCalledTimes(4);
  expect(hook.result.current.summaryResults.map((row) => row.assessment?.planId)).toEqual(["latest"]);
  expect(hook.result.current.results.map((row) => row.assessment?.planId)).toEqual(["second", "latest"]);
  await act(() => hook.result.current.expand(row, false));
  await act(async () => { await vi.advanceTimersByTimeAsync(TEST_RESULT_REFRESH_MS); });
  expect(loader).toHaveBeenCalledTimes(5);
  hook.unmount();
  await vi.advanceTimersByTimeAsync(TEST_RESULT_REFRESH_MS * 3);
  expect(loader).toHaveBeenCalledTimes(5);
});

it("pauses while editing and ignores an old response after disable", async () => {
  vi.useFakeTimers();
  let complete!: (response: ConsoleProviderProbeResponse) => void;
  const loader = vi.fn().mockReturnValueOnce(new Promise<ConsoleProviderProbeResponse>((resolve) => { complete = resolve; }))
    .mockResolvedValue(response("new"));
  const hook = renderHook(({ enabled }) => useTestPlanResults(["p"], loader, enabled), { initialProps: { enabled: true } });
  hook.rerender({ enabled: false });
  await act(async () => { complete(response("stale")); await vi.advanceTimersByTimeAsync(TEST_RESULT_REFRESH_MS * 2); });
  expect(hook.result.current.results).toEqual([]); expect(loader).toHaveBeenCalledOnce();
  hook.rerender({ enabled: true });
  await act(async () => {});
  expect(hook.result.current.results[0].assessment?.planId).toBe("new");
});
