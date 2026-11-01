import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleGeminiAuthSessionResponse } from "../../api/contracts";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { deferred } from "./BrowserConsoleApp.render-fixture";
import { useGeminiManualAddSession } from "./useGeminiManualAddSession";

const source = '{"schemaVersion":1,"provider":"aistudio-web","cookies":[]}';
function response(): ConsoleGeminiAuthSessionResponse {
  return { session: { id: "json-import", providerId: "studio-pool", targetFamily: "aistudio-web",
    status: "succeeded", message: "Offline draft only", createdAt: "now", updatedAt: "now",
    generatedDrafts: [{ providerId: "studio-pool", credential: { id: "new-account" }, secretEdits: [] }] } };
}
function setup() {
  const api = createConsoleApi(), apply = vi.fn(), error = vi.fn();
  const hook = renderHook(({ revision, grant }: { revision?: string; grant: string | null }) => useGeminiManualAddSession({
    api, managementToken: "management", applyGeminiManualAddSessionResult: apply, setError: error,
    jsonImportContext: { revision, secretGrant: grant }, t: (_zh, en) => en,
  }), { initialProps: { revision: "r1" as string | undefined, grant: "grant-1" as string | null } });
  return { api, apply, error, ...hook };
}

it("passes JSON, revision and secret grant once and never starts browser polling", async () => {
  const h = setup(); vi.mocked(h.api.createGeminiAuthSession).mockResolvedValue(response());
  await act(() => h.result.current.openGeminiManualAddDialog("aistudio-web", "studio-pool", "My account", source));
  expect(h.api.createGeminiAuthSession).toHaveBeenCalledWith("management", {
    targetFamily: "aistudio-web", providerId: "studio-pool", accountLabel: "My account", webSessionJson: source, expectedRevision: "r1",
  }, "grant-1");
  expect(h.apply).toHaveBeenCalledWith(response().session);
  expect(h.api.getGeminiAuthSession).not.toHaveBeenCalled();
  expect(h.api.completeGeminiAuthSession).not.toHaveBeenCalled();
});

it.each(["grant", "revision"])("does not submit sensitive material without %s", async change => {
  const h = setup(); h.rerender({ revision: change === "revision" ? undefined : "r1", grant: change === "grant" ? null : "grant-1" });
  await act(() => h.result.current.openGeminiManualAddDialog("aistudio-web", "studio-pool", undefined, source));
  expect(h.api.createGeminiAuthSession).not.toHaveBeenCalled(); expect(h.apply).not.toHaveBeenCalled();
});

it.each(["grant", "revision"])("discards a JSON result after %s changes", async change => {
  const h = setup(), pending = deferred<ConsoleGeminiAuthSessionResponse>();
  vi.mocked(h.api.createGeminiAuthSession).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  act(() => { request = h.result.current.openGeminiManualAddDialog("aistudio-web", "studio-pool", undefined, source); });
  h.rerender({ revision: change === "revision" ? "r2" : "r1", grant: change === "grant" ? null : "grant-1" });
  await act(async () => { pending.resolve(response()); await request; });
  expect(h.apply).not.toHaveBeenCalled(); expect(h.result.current.geminiManualAddDialogState).toBeNull();
});

it.each(["provider", "family", "draft", "waiting", "empty"])("rejects an invalid JSON import response: %s", async change => {
  const h = setup(), result = response();
  if (change === "provider") result.session.providerId = "other";
  if (change === "family") result.session.targetFamily = "gemini-web";
  if (change === "draft") result.session.generatedDrafts[0].providerId = "other";
  if (change === "waiting") result.session.status = "waiting_user";
  if (change === "empty") result.session.generatedDrafts = [];
  vi.mocked(h.api.createGeminiAuthSession).mockResolvedValueOnce(result);
  await act(() => h.result.current.openGeminiManualAddDialog("aistudio-web", "studio-pool", undefined, source));
  expect(h.apply).not.toHaveBeenCalled(); expect(h.api.getGeminiAuthSession).not.toHaveBeenCalled();
  expect(h.result.current.geminiManualAddDialogState).toBeNull();
});

it("fences an old import even when a revoked grant value is later restored", async () => {
  const h = setup(), pending = deferred<ConsoleGeminiAuthSessionResponse>();
  vi.mocked(h.api.createGeminiAuthSession).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  act(() => { request = h.result.current.openGeminiManualAddDialog("aistudio-web", "studio-pool", undefined, source); });
  h.rerender({ revision: "r1", grant: null }); h.rerender({ revision: "r1", grant: "grant-1" });
  await act(async () => { pending.resolve(response()); await request; });
  expect(h.apply).not.toHaveBeenCalled(); expect(h.result.current.geminiManualAddDialogState).toBeNull();
});

it("ignores late import errors after the secret context is revoked", async () => {
  const h = setup(), pending = deferred<ConsoleGeminiAuthSessionResponse>();
  vi.mocked(h.api.createGeminiAuthSession).mockReturnValueOnce(pending.promise);
  let request!: Promise<void>;
  act(() => { request = h.result.current.openGeminiManualAddDialog("aistudio-web", "studio-pool", undefined, source); });
  h.rerender({ revision: "r1", grant: null }); h.error.mockClear();
  await act(async () => { pending.reject(new Error("late-error")); await request; });
  expect(h.error).not.toHaveBeenCalled(); expect(h.result.current.geminiManualAddDialogState).toBeNull();
});
