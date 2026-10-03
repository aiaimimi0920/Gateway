import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteConfigResponse } from "../../api/contracts";
import { useConsoleRouteDraft } from "./useConsoleRouteDraft";

function setup(access = true) {
  const routeConfig: ConsoleRouteConfigResponse = { routeConfig: {
    revision: { id: "fixture-r1", sequence: 1, message: "fixture" }, source: "local",
    document: { providers: [{ id: "p" }], model_routes: [], aliases: {} },
    diagnostics: { diagnostics: [] }, requiresRepair: false, mutationSupported: true,
    secrets: [],
  } };
  const options = { routeConfig, hasSecretAccess: access, t: (zh: string) => zh,
    invalidateCredentialProbes: vi.fn(), accountGroupDraftRowsFromDocument: () => [],
    onRouteConfigHydrated: vi.fn(), setError: vi.fn(), setSecretDialogOpen: vi.fn() };
  return { ...renderHook(() => useConsoleRouteDraft(options)), options };
}
const connection = { type: "webdav" as const, endpoint: "https://storage.example.invalid", directory: "fixtures", username: "test-user" };
it("creates a cloud connection and its first secret patch atomically without embedding auth", () => {
  const h = setup();
  act(() => { expect(h.result.current.updateProviderStorageConnection("p", "storage", connection,
    { password: { operation: "replace", value: "synthetic-only-secret" } })).toBe(true); });
  const request = h.result.current.parseDraft();
  expect(JSON.stringify(request.document)).not.toContain("synthetic-only-secret");
  expect(request.document.providers[0]).toMatchObject({ credential_storage_connection: connection });
  expect(request.secretPatches).toContainEqual({ path: "/providers/0/credential_storage_connection/password", operation: "replace", value: "synthetic-only-secret" });
  expect(h.result.current.draftDirty).toBe(true);
});
it("requires secret access before changing either connection structure or credentials", () => {
  const h = setup(false);
  act(() => { expect(h.result.current.updateProviderStorageConnection("p", "archive", connection,
    { password: { operation: "replace", value: "synthetic-only-secret" } })).toBe(false); });
  expect(h.options.setSecretDialogOpen).toHaveBeenCalledWith(true);
  expect(h.result.current.parseDraft().document.providers[0]).not.toHaveProperty("credential_archive_connection");
  expect(h.result.current.secretPatches).toEqual([]);
});
it("does not accidentally carry uncommitted secrets to a different destination", () => {
  const h = setup();
  act(() => { h.result.current.updateProviderStorageConnection("p", "storage", connection,
    { password: { operation: "replace", value: "first-secret" } }); });
  act(() => { h.result.current.updateProviderStorageConnection("p", "storage", { ...connection, endpoint: "https://second.example.invalid" },
    { password: { operation: "replace", value: "second-secret" } }); });
  const serialized = JSON.stringify(h.result.current.parseDraft());
  expect(serialized).not.toContain("first-secret");
  expect(serialized).toContain("second-secret");
});

it("blocks raw JSON destination edits from carrying staged replacement credentials", () => {
  const h = setup();
  act(() => { h.result.current.updateProviderStorageConnection("p", "storage", connection,
    { password: { operation: "replace", value: "first-secret" } }); });
  const changed = h.result.current.editorText.replace("storage.example.invalid", "second.example.invalid");
  act(() => { h.result.current.setEditorText(changed); });
  expect(() => h.result.current.parseDraft()).toThrow("不能将未提交的凭据移到新目标");
  expect(JSON.stringify(h.result.current.secretPatches)).not.toContain("first-secret");
  act(() => { h.result.current.updateProviderStorageConnection("p", "storage", { ...connection, endpoint: "https://second.example.invalid" }, {}); });
  expect(JSON.stringify(h.result.current.parseDraft())).not.toContain("first-secret");
});
