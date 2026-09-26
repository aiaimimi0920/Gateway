import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleGeminiAuthSessionResponse } from "../../api/contracts";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useGeminiManualAddSession } from "./useGeminiManualAddSession";

function deferred() {
  let resolve!: (value: ConsoleGeminiAuthSessionResponse) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<ConsoleGeminiAuthSessionResponse>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const response = (id: string, status: "succeeded" | "waiting_user" = "succeeded"):
ConsoleGeminiAuthSessionResponse => ({ session: {
  id, providerId: "gemini-canvas", targetFamily: "gemini-canvas", status,
  message: "fixture", createdAt: "2026-09-09T00:00:00Z", updatedAt: "2026-09-09T00:00:00Z",
  generatedDrafts: [],
} });

function setup() {
  const api = createConsoleApi();
  const apply = vi.fn();
  const error = vi.fn();
  const hook = renderHook(({ token }) => useGeminiManualAddSession({
    api, managementToken: token, applyGeminiManualAddSessionResult: apply,
    setError: error, t: (_zh, en) => en,
  }), { initialProps: { token: "initial-token" } });
  return { api, apply, error, ...hook };
}

it.each(["close", "unmount", "token"] as const)("ignores a late create after %s", async (action) => {
  const h = setup();
  const pending = deferred();
  vi.mocked(h.api.createGeminiAuthSession).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  act(() => { request = h.result.current.openGeminiManualAddDialog("gemini-canvas", "old"); });
  if (action === "close") act(() => h.result.current.closeGeminiManualAddDialog());
  if (action === "unmount") h.unmount();
  if (action === "token") h.rerender({ token: "replacement-token" });
  await act(async () => { pending.resolve(response("old")); await request; });
  expect(h.apply).not.toHaveBeenCalled();
  if (action !== "unmount") expect(h.result.current.geminiManualAddDialogState).toBeNull();
});

it("preserves the newer dialog when an earlier create resolves last", async () => {
  const h = setup();
  const old = deferred();
  vi.mocked(h.api.createGeminiAuthSession).mockReturnValueOnce(old.promise)
    .mockResolvedValueOnce(response("new"));
  let request!: Promise<void>;
  act(() => { request = h.result.current.openGeminiManualAddDialog("gemini-canvas", "old"); });
  await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "new"));
  await act(async () => { old.resolve(response("old")); await request; });
  expect(h.result.current.geminiManualAddDialogState?.session?.id).toBe("new");
  expect(h.apply.mock.calls.map(([session]) => session.id)).toEqual(["new"]);
});

it("does not apply an in-flight refresh after closing", async () => {
  const h = setup();
  const pending = deferred();
  vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(response("active", "waiting_user"));
  vi.mocked(h.api.getGeminiAuthSession).mockReturnValueOnce(pending.promise);
  await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "active"));
  act(() => h.result.current.closeGeminiManualAddDialog());
  await act(async () => { pending.resolve(response("active")); await pending.promise; });
  expect(h.apply).not.toHaveBeenCalled();
  expect(h.result.current.geminiManualAddDialogState).toBeNull();
});

it("does not report a completion error after closing", async () => {
  const h = setup();
  const pending = deferred();
  vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(response("active", "waiting_user"));
  vi.mocked(h.api.getGeminiAuthSession).mockResolvedValue(response("active", "waiting_user"));
  vi.mocked(h.api.completeGeminiAuthSession).mockReturnValueOnce(pending.promise);
  await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "active"));
  let request!: Promise<void>;
  act(() => { request = h.result.current.requestGeminiManualAddCompletion(); });
  act(() => h.result.current.closeGeminiManualAddDialog());
  h.error.mockClear();
  await act(async () => { pending.reject(new Error("late completion")); await request; });
  expect(h.error).not.toHaveBeenCalled();
});

it("reports a live refresh failure without rejecting its fire-and-forget request", async () => {
  const h = setup();
  vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(response("active", "waiting_user"));
  vi.mocked(h.api.getGeminiAuthSession).mockRejectedValueOnce(new Error("refresh unavailable"));
  await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "active"));
  expect(h.error).toHaveBeenLastCalledWith("refresh unavailable");
  expect(h.apply).not.toHaveBeenCalled();
  h.unmount();
});

it("clears the scheduled poll on unmount", async () => {
  vi.useFakeTimers();
  try {
    const h = setup();
    vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(response("active", "waiting_user"));
    vi.mocked(h.api.getGeminiAuthSession).mockResolvedValue(response("active", "waiting_user"));
    await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "active"));
    expect(h.api.getGeminiAuthSession).toHaveBeenCalledTimes(1);
    h.unmount();
    await act(() => vi.advanceTimersByTimeAsync(3000));
    expect(h.api.getGeminiAuthSession).toHaveBeenCalledTimes(1);
  } finally {
    vi.useRealTimers();
  }
});

it("does not replace a completed session with an older pending refresh", async () => {
  const h = setup();
  const pending = deferred();
  vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(response("active", "waiting_user"));
  vi.mocked(h.api.getGeminiAuthSession).mockReturnValueOnce(pending.promise);
  vi.mocked(h.api.completeGeminiAuthSession).mockResolvedValueOnce(response("active"));
  await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "active"));
  await act(() => h.result.current.requestGeminiManualAddCompletion());
  expect(h.result.current.geminiManualAddDialogState?.session?.status).toBe("succeeded");
  h.apply.mockClear();
  await act(async () => { pending.resolve(response("active", "waiting_user")); await pending.promise; });
  expect(h.result.current.geminiManualAddDialogState?.session?.status).toBe("succeeded");
  expect(h.apply).not.toHaveBeenCalled();
  h.unmount();
});

it("pauses polling and duplicate completion while a manual completion is pending", async () => {
  vi.useFakeTimers();
  try {
    const h = setup();
    const pending = deferred();
    vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(response("active", "waiting_user"));
    vi.mocked(h.api.getGeminiAuthSession).mockResolvedValue(response("active", "waiting_user"));
    vi.mocked(h.api.completeGeminiAuthSession).mockReturnValueOnce(pending.promise);
    await act(() => h.result.current.openGeminiManualAddDialog("gemini-canvas", "active"));
    let completion!: Promise<void>;
    act(() => { completion = h.result.current.requestGeminiManualAddCompletion(); });
    await act(() => h.result.current.requestGeminiManualAddCompletion());
    await act(() => vi.advanceTimersByTimeAsync(3000));
    expect(h.api.completeGeminiAuthSession).toHaveBeenCalledTimes(1);
    expect(h.api.getGeminiAuthSession).toHaveBeenCalledTimes(1);
    await act(async () => { pending.resolve(response("active", "waiting_user")); await completion; });
    expect(h.api.getGeminiAuthSession).toHaveBeenCalledTimes(2);
    expect(h.result.current.geminiManualAddDialogState?.busy).toBe(false);
    h.unmount();
  } finally {
    vi.useRealTimers();
  }
});
