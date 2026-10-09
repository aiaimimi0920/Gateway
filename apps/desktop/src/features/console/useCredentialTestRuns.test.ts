import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleProviderProbeResponse, CredentialTestScope } from "../../api/contracts";
import type { TestInvocation } from "./credentialTestSelection";
import { useCredentialTestRuns } from "./useCredentialTestRuns";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";

const targets: CredentialTestScope[] = [{ kind: "subpool", providerId: "p", id: "free" }, { kind: "subpool", providerId: "p", id: "plus" }];
const requests: TestInvocation[] = targets.map((scope) => ({ providerId: scope.providerId, request: { scope: { kind: "subpool", id: "id" in scope ? scope.id : "" } } }));

it("executes selected scopes serially and retains each response", async () => {
  const response = await createConsoleApi().probeProvider("token", "grant", "p");
  let complete!: (value: ConsoleProviderProbeResponse) => void;
  const probe = vi.fn().mockReturnValueOnce(new Promise((resolve) => { complete = resolve; })).mockResolvedValue(response);
  const h = renderHook(() => useCredentialTestRuns("both", targets, probe));
  let pending!: Promise<void>;
  act(() => { pending = h.result.current.run(requests); });
  expect(probe).toHaveBeenCalledTimes(1);
  expect(h.result.current.busy).toBe(true);
  await act(async () => { complete(response); await pending; });
  expect(probe).toHaveBeenCalledTimes(2);
  expect(probe.mock.calls.map(([request]) => request.scope.id)).toEqual(["free", "plus"]);
  expect(h.result.current.responses).toHaveLength(2);
  expect(h.result.current.busy).toBe(false);
});

it("does not send a second scope after close or failed authorization", async () => {
  const response = await createConsoleApi().probeProvider("token", "grant", "p");
  let complete!: (value: ConsoleProviderProbeResponse) => void;
  const probe = vi.fn().mockReturnValue(new Promise((resolve) => { complete = resolve; }));
  const h = renderHook(() => useCredentialTestRuns("both", targets, probe));
  let pending!: Promise<void>;
  act(() => { pending = h.result.current.run(requests); });
  h.unmount();
  await act(async () => { complete(response); await pending; });
  expect(probe).toHaveBeenCalledTimes(1);
  const refused = vi.fn().mockResolvedValue(undefined);
  const other = renderHook(() => useCredentialTestRuns("both", targets, refused));
  await act(() => other.result.current.run(requests));
  expect(refused).toHaveBeenCalledTimes(1);
  expect(other.result.current.busy).toBe(false);
});

it("stops admitting targets when the whole round deadline expires", async () => {
  const response = await createConsoleApi().probeProvider("token", "grant", "p");
  const clock = vi.spyOn(performance, "now").mockReturnValue(0);
  const probe = vi.fn().mockImplementation(async () => { clock.mockReturnValue(300_000); return response; });
  const h = renderHook(() => useCredentialTestRuns("both", targets, probe));
  try {
    await act(() => h.result.current.run(requests));
    expect(probe).toHaveBeenCalledTimes(1);
    expect(h.result.current.error).toBe("本轮超时");
    expect(h.result.current.busy).toBe(false);
  } finally { clock.mockRestore(); }
});
